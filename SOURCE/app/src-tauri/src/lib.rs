//! Couche Tauri : expose les moteurs Rust à l'interface. Aucun calcul mathématique ici — tout vient
//! de `smi-core` (lois exactes, simulation, validation) et `smi-data` (base, imports, collecte).
//! Les travaux longs (simulations, validation, collecte) tournent dans des threads dédiés suivis par
//! un identifiant de tâche : progression, pause, reprise, annulation ; l'interface ne bloque jamais.

use serde::Deserialize;
use serde_json::{json, Value};
use smi_core::dist::Distribution;
use smi_core::generator::{generate, GeneratorSpec};
use smi_core::reels::{self, ReelGame};
use smi_core::sim::{self, Control, SessionConfig, SimConfig};
use smi_core::stats::compute;
use smi_core::validate::{validate, ModelStatus};
use smi_core::volatility::{classify, VolInputs};
use smi_data::market::{self, MarketFilter};
use smi_data::settings::AppSettings;
use smi_data::{collector, documents, models, report, Db, Paths};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::State;

type R<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct Job {
    kind: String,
    label: String,
    started_at: String,
    control: Arc<Control>,
    progress: Mutex<(f64, String)>,
    status: Mutex<String>,
    result: Mutex<Option<Value>>,
    error: Mutex<Option<String>>,
}

pub struct AppState {
    paths: Paths,
    db: Arc<Db>,
    jobs: Mutex<HashMap<u64, Arc<Job>>>,
    next: AtomicU64,
    /// Pack de départ : « absent » (déjà installé ou pas de pack), « en cours », « terminé », « erreur ».
    starter: Arc<Mutex<Value>>,
}

impl AppState {
    fn settings(&self) -> AppSettings {
        AppSettings::load(&self.db)
    }

    fn spawn<F>(&self, kind: &str, label: &str, f: F) -> u64
    where
        F: FnOnce(&Job) -> Result<Value, String> + Send + 'static,
    {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let job = Arc::new(Job {
            kind: kind.into(),
            label: label.into(),
            started_at: smi_data::now(),
            control: Arc::new(Control::default()),
            progress: Mutex::new((0.0, "Démarrage".into())),
            status: Mutex::new("running".into()),
            result: Mutex::new(None),
            error: Mutex::new(None),
        });
        self.jobs.lock().unwrap().insert(id, job.clone());
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&job)));
            let cancelled = job.control.cancel.load(Ordering::Relaxed);
            match r {
                Ok(Ok(v)) => {
                    *job.result.lock().unwrap() = Some(v);
                    *job.status.lock().unwrap() = if cancelled { "cancelled".into() } else { "done".into() };
                    *job.progress.lock().unwrap() = (1.0, if cancelled { "Annulée".into() } else { "Terminé".into() });
                }
                Ok(Err(e)) => {
                    *job.error.lock().unwrap() = Some(e);
                    *job.status.lock().unwrap() = if cancelled { "cancelled".into() } else { "error".into() };
                }
                Err(_) => {
                    *job.error.lock().unwrap() = Some("erreur interne (panique) : voir le journal".into());
                    *job.status.lock().unwrap() = "error".into();
                }
            }
        });
        id
    }
}

impl Job {
    fn set(&self, p: f64, msg: &str) {
        *self.progress.lock().unwrap() = (p.clamp(0.0, 1.0), msg.into());
    }
}

// ---------------------------------------------------------------------------------------------
// Application, réglages, tableau de bord
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn app_info(st: State<AppState>) -> R<Value> {
    let s = st.settings();
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "engine": smi_core::ENGINE_VERSION,
        "paths": st.paths,
        "cores": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        "threads": s.threads(),
        "schema_version": st.db.schema_version(),
        "robots_notice": collector::ROBOTS_NOTICE,
    }))
}

#[tauri::command]
fn structure_check(st: State<AppState>) -> R<Value> {
    let dirs = st.paths.ensure().map_err(err)?;
    let files = ["PROJECT/MASTER_PROMPT.md", "PROJECT/AUDIT.md", "PROJECT/DECISIONS.md", "PROJECT/ROADMAP.md", "DATABASE/smi.sqlite"];
    let checks: Vec<Value> = files.iter().map(|f| json!({"path": f, "exists": st.paths.root.join(f).exists()})).collect();
    Ok(json!({"dirs": dirs, "files": checks, "schema_version": st.db.schema_version()}))
}

#[tauri::command]
fn dashboard(st: State<AppState>) -> R<Value> {
    serde_json::to_value(models::dashboard(&st.db).map_err(err)?).map_err(err)
}

#[tauri::command]
fn alert_resolve(st: State<AppState>, id: i64) -> R<()> {
    models::resolve_alert(&st.db, id).map_err(err)
}

#[tauri::command]
fn settings_get(st: State<AppState>) -> R<Value> {
    let s = st.settings();
    Ok(json!({"settings": s, "threads": s.threads(), "cores": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)}))
}

#[tauri::command]
fn settings_save(st: State<AppState>, settings: AppSettings) -> R<Value> {
    let mut s = settings;
    s.collector.delay_ms = s.collector.delay_ms.max(1_000);
    s.save(&st.db).map_err(err)?;
    st.db.log("réglages", "Réglages enregistrés");
    Ok(json!({"settings": s, "threads": s.threads()}))
}

#[tauri::command]
fn collector_enable(st: State<AppState>, enabled: bool) -> R<Value> {
    let mut s = st.settings();
    s.collector.enabled = enabled;
    s.collector.accepted_at = if enabled { Some(smi_data::now()) } else { None };
    s.save(&st.db).map_err(err)?;
    st.db.log("collecte", if enabled { "Collecte StakeCruncher ACTIVÉE par l'utilisateur (avertissement robots.txt accepté)" } else { "Collecte StakeCruncher désactivée" });
    Ok(json!({"settings": s}))
}

// ---------------------------------------------------------------------------------------------
// Tâches
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn job_status(st: State<AppState>, id: u64) -> R<Value> {
    let jobs = st.jobs.lock().unwrap();
    let j = jobs.get(&id).ok_or("tâche inconnue")?;
    let (p, msg) = j.progress.lock().unwrap().clone();
    let status = j.status.lock().unwrap().clone();
    let paused = j.control.pause.load(Ordering::Relaxed);
    Ok(json!({
        "id": id, "kind": j.kind, "label": j.label, "started_at": j.started_at,
        "status": if status == "running" && paused { "paused".to_string() } else { status.clone() },
        "progress": p, "message": msg,
        "result": if status == "done" || status == "cancelled" { j.result.lock().unwrap().clone() } else { None },
        "error": j.error.lock().unwrap().clone(),
    }))
}

#[tauri::command]
fn job_control(st: State<AppState>, id: u64, action: String) -> R<()> {
    let jobs = st.jobs.lock().unwrap();
    let j = jobs.get(&id).ok_or("tâche inconnue")?;
    match action.as_str() {
        "pause" => j.control.pause.store(true, Ordering::Relaxed),
        "resume" => j.control.pause.store(false, Ordering::Relaxed),
        "cancel" => {
            j.control.cancel.store(true, Ordering::Relaxed);
            j.control.pause.store(false, Ordering::Relaxed);
        }
        _ => return Err(format!("action inconnue : {action}")),
    }
    Ok(())
}

#[tauri::command]
fn jobs_list(st: State<AppState>) -> R<Value> {
    let jobs = st.jobs.lock().unwrap();
    let mut v: Vec<Value> = jobs
        .iter()
        .map(|(id, j)| json!({"id": id, "kind": j.kind, "label": j.label, "status": j.status.lock().unwrap().clone(), "progress": j.progress.lock().unwrap().0, "started_at": j.started_at}))
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x["id"].as_u64().unwrap_or(0)));
    Ok(Value::Array(v))
}

// ---------------------------------------------------------------------------------------------
// Marché
// ---------------------------------------------------------------------------------------------

#[tauri::command]
async fn market_query(st: State<'_, AppState>, filter: MarketFilter) -> R<Value> {
    let s = st.settings();
    let (rows, basis) = market::market_rows(&st.db, &filter).map_err(err)?;
    let aggregates = market::aggregates(&rows, s.market.min_sample);
    let providers = market::providers_summary(&rows);
    let sources: Vec<Value> = {
        let c = st.db.conn();
        let mut q = c
            .prepare("SELECT s.name, s.kind, s.note, c.id, COALESCE(c.data_date, c.started_at), c.status, c.items, c.errors, COALESCE(c.message,'') FROM collections c JOIN sources s ON s.id = c.source_id ORDER BY c.id DESC LIMIT 30")
            .map_err(err)?;
        let v = q
            .query_map([], |r| {
                Ok(json!({"source": r.get::<_, String>(0)?, "kind": r.get::<_, String>(1)?, "note": r.get::<_, Option<String>>(2)?, "id": r.get::<_, i64>(3)?, "date": r.get::<_, String>(4)?, "status": r.get::<_, String>(5)?, "items": r.get::<_, i64>(6)?, "errors": r.get::<_, i64>(7)?, "message": r.get::<_, String>(8)?}))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        v
    };
    Ok(json!({"rows": rows, "basis": basis, "aggregates": aggregates, "providers": providers, "sources": sources}))
}

#[tauri::command]
fn market_ranking(st: State<AppState>) -> R<Value> {
    serde_json::to_value(market::ranking(&st.db).map_err(err)?).map_err(err)
}

#[tauri::command]
async fn mode_full_stats(st: State<'_, AppState>, mode_id: i64) -> R<Value> {
    let s = st.settings();
    match market::load_mode_distribution(&st.db, mode_id).map_err(err)? {
        Some(d) => {
            let stats = compute(&d, &s.stats);
            let vol = classify(&VolInputs::from_stats(&stats), &s.volatility);
            Ok(json!({"stats": stats, "vol": vol}))
        }
        None => Ok(json!({"stats": null, "reason": "Pas de lookup table enregistrée pour ce mode : seuls les chiffres importés sont disponibles."})),
    }
}

#[tauri::command]
async fn import_slotlab(st: State<'_, AppState>, path: String) -> R<Value> {
    let s = st.settings();
    serde_json::to_value(market::import_slotlab_snapshot(&st.db, &s, std::path::Path::new(&path)).map_err(err)?).map_err(err)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn import_lut(st: State<'_, AppState>, path: String, slot_name: String, provider: String, mode: String, cost: f64, ours: bool) -> R<Value> {
    let s = st.settings();
    serde_json::to_value(market::import_lut_file(&st.db, &st.paths, &s, std::path::Path::new(&path), &slot_name, &provider, &mode, cost, ours).map_err(err)?).map_err(err)
}

#[tauri::command]
fn collect_start(st: State<AppState>) -> R<u64> {
    let s = st.settings();
    if !s.collector.enabled {
        return Err(format!("Collecte désactivée. {}", collector::ROBOTS_NOTICE));
    }
    let (db, paths) = (st.db.clone(), st.paths.clone());
    Ok(st.spawn("collect", "Collecte StakeCruncher", move |job| {
        let r = collector::collect(&db, &paths, &s, &|m, p| job.set(p, m), &job.control.cancel).map_err(err)?;
        serde_json::to_value(r).map_err(err)
    }))
}

// ---------------------------------------------------------------------------------------------
// Générateur et bibliothèque de modèles
// ---------------------------------------------------------------------------------------------

#[tauri::command]
async fn generator_peers(st: State<'_, AppState>, max_win: f64, price: f64) -> R<Value> {
    let s = st.settings();
    let (peers, basis) = market::peers(&st.db, &s, max_win, price).map_err(err)?;
    let aggregates = market::aggregates(&peers, s.market.min_sample);
    Ok(json!({"peers": peers, "basis": basis, "aggregates": aggregates, "providers": market::providers_summary(&peers)}))
}

#[tauri::command]
async fn generate_preview(st: State<'_, AppState>, spec: GeneratorSpec) -> R<Value> {
    let s = st.settings();
    let db = st.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let g = generate(&spec, &s.volatility).map_err(err)?;
        let mv = models::ModelVersion {
            model_id: "aperçu".into(),
            name: spec.name.clone(),
            version: 0,
            created_at: smi_data::now(),
            spec: spec.clone(),
            params: Some(g.params.clone()),
            notes: g.notes.clone(),
            fingerprint: g.distribution.fingerprint(),
            distribution: g.distribution.clone(),
            change_note: String::new(),
            status: ModelStatus::Draft,
        };
        let (stats, base, bonus) = report::model_stats(&mv, &s);
        let vol = classify(&VolInputs::from_stats(&stats), &s.volatility);
        let market = market::market_context(&db, &s, &stats, spec.max_win, spec.price).map_err(err)?;
        Ok(json!({"params": g.params, "notes": g.notes, "outcomes": g.distribution.atoms.len(), "stats": stats, "base": base, "bonus": bonus, "vol": vol, "market": market}))
    })
    .await
    .map_err(err)?
}

#[tauri::command]
async fn model_save(st: State<'_, AppState>, spec: GeneratorSpec, model_id: Option<String>, note: String) -> R<Value> {
    let s = st.settings();
    let g = generate(&spec, &s.volatility).map_err(err)?;
    let (id, v) = match model_id {
        Some(id) => {
            let v = models::add_version(&st.db, &id, &g, &note).map_err(err)?;
            (id, v)
        }
        None => models::create_model(&st.db, &g, &["Math Generator".into()], &note).map_err(err)?,
    };
    st.db.log("modèle", &format!("{} v{} enregistré", spec.name, v));
    Ok(json!({"id": id, "version": v}))
}

#[tauri::command]
fn models_list(st: State<AppState>) -> R<Value> {
    serde_json::to_value(models::list_models(&st.db).map_err(err)?).map_err(err)
}

#[tauri::command]
async fn model_get(st: State<'_, AppState>, id: String, version: Option<u32>) -> R<Value> {
    let s = st.settings();
    let mv = models::get_version(&st.db, &id, version).map_err(err)?;
    let (stats, base, bonus) = report::model_stats(&mv, &s);
    let vol = classify(&VolInputs::from_stats(&stats), &s.volatility);
    let validation = models::latest_validation(&st.db, &id, mv.version).map_err(err)?;
    let sim = models::latest_model_sim(&st.db, &id, mv.version, &mv.fingerprint).map_err(err)?;
    let versions = models::versions(&st.db, &id).map_err(err)?;
    let sims = models::list_simulations(&st.db, Some(&id), 50).map_err(err)?;
    let mut light = serde_json::to_value(&mv).map_err(err)?;
    light["distribution"] = json!({"outcomes": mv.distribution.atoms.len(), "classes": mv.distribution.classes, "cap": mv.distribution.cap, "cost": mv.distribution.cost});
    Ok(json!({"model": light, "stats": stats, "base": base, "bonus": bonus, "vol": vol, "validation": validation, "sim": sim, "versions": versions, "simulations": sims}))
}

#[tauri::command]
async fn model_diff(st: State<'_, AppState>, id: String, a: u32, b: u32) -> R<Value> {
    let s = st.settings();
    let va = models::get_version(&st.db, &id, Some(a)).map_err(err)?;
    let vb = models::get_version(&st.db, &id, Some(b)).map_err(err)?;
    let sa = compute(&va.distribution, &s.stats);
    let sb = compute(&vb.distribution, &s.stats);
    serde_json::to_value(models::diff(&va, &vb, &sa, &sb)).map_err(err)
}

#[tauri::command]
fn model_set_status(st: State<AppState>, id: String, version: u32, status: ModelStatus) -> R<()> {
    models::set_status(&st.db, &id, version, status).map_err(err)
}

// ---------------------------------------------------------------------------------------------
// Simulations
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SimTarget {
    Model { id: String, version: Option<u32> },
    Mode { mode_id: i64 },
    Reels { game: Box<ReelGame>, buy: Option<(usize, f64)> },
}

fn load_target(db: &Db, t: &SimTarget) -> Result<(Distribution, String, Option<(String, u32)>), String> {
    match t {
        SimTarget::Model { id, version } => {
            let mv = models::get_version(db, id, *version).map_err(err)?;
            Ok((mv.distribution, format!("{} v{}", mv.name, mv.version), Some((id.clone(), mv.version))))
        }
        SimTarget::Mode { mode_id } => {
            let d = market::load_mode_distribution(db, *mode_id).map_err(err)?.ok_or("ce mode n'a pas de lookup table enregistrée")?;
            let label: String = db
                .conn()
                .query_row("SELECT s.name || ' — ' || m.name FROM modes m JOIN slots s ON s.id = m.slot_id WHERE m.id = ?1", [mode_id], |r| r.get(0))
                .map_err(err)?;
            Ok((d, label, None))
        }
        SimTarget::Reels { .. } => Err("cible de rouleaux : utiliser la simulation de règles".into()),
    }
}

#[tauri::command]
fn sim_start(st: State<AppState>, target: SimTarget, spins: u64, seed: u64, threads: Option<usize>) -> R<u64> {
    let s = st.settings();
    let threads = threads.filter(|&t| t > 0).unwrap_or_else(|| s.threads());
    let db = st.db.clone();
    if let SimTarget::Reels { game, buy } = target {
        let game = *game;
        let limit = s.enumeration_limit;
        return Ok(st.spawn("reels", &format!("Règles « {} » — {} rounds", game.name, spins), move |job| {
            job.set(0.0, "Calcul exact du jeu de base");
            let exact = reels::exact_base(&game, limit).map_err(err)?;
            let ctl = job.control.clone();
            let r = std::thread::scope(|sc| {
                let h = sc.spawn(|| reels::simulate(&game, spins, seed, threads, buy, &ctl));
                while !h.is_finished() {
                    let d = ctl.done.load(Ordering::Relaxed);
                    job.set(d as f64 / spins.max(1) as f64, &format!("{} / {} rounds", smi_core::stats::group(d), smi_core::stats::group(spins)));
                    std::thread::sleep(std::time::Duration::from_millis(150));
                }
                h.join()
            })
            .map_err(|_| "erreur interne".to_string())?
            .map_err(err)?;
            let stats = compute(&r.distribution, &s.stats);
            let cfg = json!({"spins": spins, "seed": seed, "threads": threads, "buy": buy, "game": game.name});
            let result = json!({"reel": {"rounds": r.rounds, "seed": r.seed, "trigger_rate": r.trigger_rate, "bonus_rounds": r.bonus_rounds, "free_spins_played": r.free_spins_played, "elapsed_ms": r.elapsed_ms, "price": r.price, "buy": r.buy_scatters}, "stats": stats, "exact_base": exact});
            let id = models::save_simulation(&db, "reels", None, None, &game.name, &cfg, &result, r.rounds, Some(stats.rtp), r.rounds == spins, r.elapsed_ms, Some(&r.distribution.fingerprint())).map_err(err)?;
            Ok(json!({"sim_id": id, "kind": "reels", "result": result}))
        }));
    }
    let (d, label, model) = load_target(&db, &target)?;
    let kind = if model.is_some() { "model" } else { "mode" };
    Ok(st.spawn("sim", &format!("{label} — {} spins", smi_core::stats::group(spins)), move |job| {
        let cfg = SimConfig { spins, seed, threads, chunk_size: 1 << 20 };
        let r = sim::run(&d, &cfg, &job.control, &s.stats, &|done, total| job.set(done as f64 / total.max(1) as f64, &format!("{} / {} spins", smi_core::stats::group(done), smi_core::stats::group(total))));
        let id = models::save_simulation(
            &db,
            kind,
            model.as_ref().map(|m| m.0.as_str()),
            model.as_ref().map(|m| m.1),
            &label,
            &serde_json::to_value(&cfg).map_err(err)?,
            &serde_json::to_value(&r).map_err(err)?,
            r.spins_done,
            Some(r.comparison.rtp_sim),
            r.completed,
            r.elapsed_ms,
            Some(&r.fingerprint),
        )
        .map_err(err)?;
        db.log("simulation", &format!("{label} : {} spins, RTP simulé {:.4} % (z = {:.2})", r.spins_done, r.comparison.rtp_sim * 100.0, r.comparison.rtp_z));
        let theory = compute(&d, &s.stats);
        Ok(json!({"sim_id": id, "kind": kind, "result": r, "theory": theory}))
    }))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn sessions_start(st: State<AppState>, target: SimTarget, rounds: u64, sessions: u64, seed: u64, bankroll: Option<f64>, big_win: f64) -> R<u64> {
    let s = st.settings();
    let (d, label, _) = load_target(&st.db, &target)?;
    let threads = s.threads();
    Ok(st.spawn("sessions", &format!("Sessions {label}"), move |job| {
        let cfg = SessionConfig { rounds, sessions, seed, threads, bankroll, big_win };
        let ctl = job.control.clone();
        let r = std::thread::scope(|sc| {
            let h = sc.spawn(|| sim::run_sessions(&d, &cfg, &ctl));
            while !h.is_finished() {
                let done = ctl.done.load(Ordering::Relaxed);
                job.set(done as f64 / sessions.max(1) as f64, &format!("{} / {} sessions", done, sessions));
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            h.join()
        })
        .map_err(|_| "erreur interne".to_string())?;
        serde_json::to_value(r).map_err(err)
    }))
}

#[tauri::command]
fn simulations_list(st: State<AppState>, model_id: Option<String>) -> R<Value> {
    serde_json::to_value(models::list_simulations(&st.db, model_id.as_deref(), 200).map_err(err)?).map_err(err)
}

#[tauri::command]
fn simulation_get(st: State<AppState>, id: i64) -> R<Value> {
    let (c, r, l) = models::get_simulation(&st.db, id).map_err(err)?;
    Ok(json!({"config": c, "result": r, "label": l}))
}

/// Compare des simulations et désigne la plus proche des objectifs de son modèle (ou des objectifs donnés).
#[tauri::command]
fn simulations_compare(st: State<AppState>, ids: Vec<i64>) -> R<Value> {
    let mut out = Vec::new();
    for id in ids {
        let (cfg, r, label) = models::get_simulation(&st.db, id).map_err(err)?;
        let model_id: Option<(Option<String>, Option<u32>)> = st
            .db
            .conn()
            .query_row("SELECT model_id, version FROM simulations WHERE id = ?1", [id], |x| Ok((x.get(0)?, x.get(1)?)))
            .ok();
        let spec = match model_id {
            Some((Some(mid), Some(v))) => models::get_version(&st.db, &mid, Some(v)).ok().map(|m| (m.spec, m.params)),
            _ => None,
        };
        let emp = r.get("empirical").or_else(|| r.get("stats")).cloned().unwrap_or(Value::Null);
        let g = |k: &str| emp.get(k).and_then(|v| v.as_f64());
        let bonus_one_in = emp.get("bonus").and_then(|b| b.get("one_in")).and_then(|v| v.as_f64());
        let max_one_in = emp.get("max_win").and_then(|b| b.get("one_in")).and_then(|v| v.as_f64());
        let distance = spec.as_ref().map(|(sp, params)| {
            let mut d = 0.0;
            let rel = |a: Option<f64>, t: f64| a.map_or(1.0, |a| ((a - t) / t).abs());
            d += rel(g("rtp"), sp.rtp) * 10.0;
            d += rel(g("hit_rate"), sp.hit_rate);
            d += rel(bonus_one_in, sp.bonus_one_in);
            if let Some(t) = params.as_ref().and_then(|p| p.sd_target) {
                d += rel(g("sd_per_price"), t);
            }
            d
        });
        out.push(json!({
            "id": id, "label": label, "config": cfg,
            "rtp": g("rtp"), "hit_rate": g("hit_rate"), "sd_per_price": g("sd_per_price"), "median": g("median"),
            "bonus_one_in": bonus_one_in, "bonus_mean": emp.get("bonus").and_then(|b| b.get("mean")).cloned(),
            "bonus_median": emp.get("bonus").and_then(|b| b.get("median")).cloned(),
            "max_observed": r.get("biggest_win").cloned(), "max_one_in": max_one_in,
            "categories": emp.get("categories").cloned(), "rtp_z": r.get("comparison").and_then(|c| c.get("rtp_z")).cloned(),
            "spins": r.get("spins_done").cloned().or_else(|| r.get("reel").and_then(|x| x.get("rounds")).cloned()),
            "targets": spec.as_ref().map(|(sp, p)| json!({"rtp": sp.rtp, "hit_rate": sp.hit_rate, "bonus_one_in": sp.bonus_one_in, "sd_target": p.as_ref().and_then(|p| p.sd_target), "volatility": sp.volatility, "max_win": sp.max_win})),
            "distance": distance,
        }));
    }
    let best = out.iter().filter_map(|v| Some((v["id"].as_i64()?, v["distance"].as_f64()?))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|x| x.0);
    Ok(json!({"rows": out, "closest": best, "method": "distance = 10·|ΔRTP|/RTP + |Δhit|/hit + |Δbonus|/bonus + |Δσ|/σ (écarts relatifs aux objectifs du modèle simulé)"}))
}

// ---------------------------------------------------------------------------------------------
// Validation, prompt, exports
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn validate_start(st: State<AppState>, id: String, version: Option<u32>, spins: u64, seed: u64) -> R<u64> {
    let s = st.settings();
    let db = st.db.clone();
    let mv = models::get_version(&db, &id, version).map_err(err)?;
    let label = format!("Validation {} v{}", mv.name, mv.version);
    Ok(st.spawn("validate", &label, move |job| {
        let stats = compute(&mv.distribution, &s.stats);
        let spins = spins.max(s.tolerances.sim_min_spins);
        let existing = models::latest_model_sim(&db, &id, mv.version, &mv.fingerprint).map_err(err)?.filter(|r| r.spins_done >= spins);
        let sim = match existing {
            Some(r) => {
                job.set(0.85, "Simulation existante réutilisée (même empreinte)");
                r
            }
            None => {
                let cfg = SimConfig { spins, seed, threads: s.threads(), chunk_size: 1 << 20 };
                let r = sim::run(&mv.distribution, &cfg, &job.control, &s.stats, &|d, t| job.set(0.85 * d as f64 / t.max(1) as f64, &format!("Simulation {} / {}", smi_core::stats::group(d), smi_core::stats::group(t))));
                models::save_simulation(&db, "model", Some(&id), Some(mv.version), &format!("{} v{}", mv.name, mv.version), &serde_json::to_value(&cfg).map_err(err)?, &serde_json::to_value(&r).map_err(err)?, r.spins_done, Some(r.comparison.rtp_sim), r.completed, r.elapsed_ms, Some(&r.fingerprint)).map_err(err)?;
                r
            }
        };
        if job.control.cancel.load(Ordering::Relaxed) {
            return Err("validation annulée".into());
        }
        job.set(0.9, "Reproductibilité (mêmes graines, 1 puis 3 threads)");
        let rp = sim::reproducibility(&mv.distribution, seed, 400_000);
        job.set(0.97, "Contrôles des 9 étapes");
        let rep = validate(&mv.distribution, &stats, &report::targets(&mv), Some(&sim), Some(&rp), &s.tolerances, &smi_data::now());
        models::save_validation(&db, &id, mv.version, &rep).map_err(err)?;
        if rep.failed > 0 {
            db.alert("ERROR", "validation", &format!("{} v{} : {} contrôle(s) en échec", mv.name, mv.version, rep.failed));
        }
        db.log("validation", &format!("{} v{} : {}", mv.name, mv.version, rep.status.label()));
        serde_json::to_value(rep).map_err(err)
    }))
}

#[tauri::command]
async fn prompt_build(st: State<'_, AppState>, id: String, version: Option<u32>) -> R<Value> {
    let s = st.settings();
    let mv = models::get_version(&st.db, &id, version).map_err(err)?;
    let md = report::prompt_for(&st.db, &s, &mv).map_err(err)?;
    let validation = models::latest_validation(&st.db, &id, mv.version).map_err(err)?;
    Ok(json!({"markdown": md, "status": validation.map(|v| v.status), "name": mv.name, "version": mv.version}))
}

#[tauri::command]
async fn export_files(st: State<'_, AppState>, id: String, version: Option<u32>, what: String) -> R<Value> {
    let s = st.settings();
    let mv = models::get_version(&st.db, &id, version).map_err(err)?;
    let base = format!("{}_v{}", market::slugify(&mv.name), mv.version);
    let mut written = Vec::new();
    match what.as_str() {
        "prompt_md" | "prompt_txt" => {
            let md = report::prompt_for(&st.db, &s, &mv).map_err(err)?;
            let ext = if what == "prompt_md" { "md" } else { "txt" };
            let p = st.paths.exports.join("prompts").join(format!("{base}_prompt.{ext}"));
            std::fs::write(&p, &md).map_err(err)?;
            models::save_prompt(&st.db, &id, mv.version, &md, Some(&p.display().to_string())).map_err(err)?;
            written.push(p);
        }
        "model_json" => {
            let (lut, js) = report::export_model(&st.paths, &s, &mv).map_err(err)?;
            written.push(js);
            written.push(lut);
        }
        "validation" => {
            let r = models::latest_validation(&st.db, &id, mv.version).map_err(err)?.ok_or("aucune validation pour cette version")?;
            let md = st.paths.exports.join("reports").join(format!("{base}_validation.md"));
            let js = st.paths.exports.join("reports").join(format!("{base}_validation.json"));
            std::fs::write(&md, report::validation_markdown(&mv.name, mv.version, &r)).map_err(err)?;
            std::fs::write(&js, serde_json::to_string_pretty(&r).map_err(err)?).map_err(err)?;
            written.push(md);
            written.push(js);
        }
        _ => return Err(format!("export inconnu : {what}")),
    }
    st.db.log("export", &format!("{} : {}", what, written.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")));
    Ok(json!({"files": written}))
}

// ---------------------------------------------------------------------------------------------
// Moteur de règles
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn reels_demo() -> R<Value> {
    serde_json::to_value(reels::demo_game()).map_err(err)
}

#[tauri::command]
async fn reels_exact(st: State<'_, AppState>, game: ReelGame) -> R<Value> {
    let limit = st.settings().enumeration_limit;
    tauri::async_runtime::spawn_blocking(move || serde_json::to_value(reels::exact_base(&game, limit).map_err(err)?).map_err(err)).await.map_err(err)?
}

// ---------------------------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn docs_list(st: State<AppState>) -> R<Value> {
    serde_json::to_value(documents::list(&st.db).map_err(err)?).map_err(err)
}

#[tauri::command]
async fn docs_import(st: State<'_, AppState>, paths: Vec<String>) -> R<Value> {
    let mut out = Vec::new();
    for p in paths {
        match documents::import(&st.db, &st.paths, std::path::Path::new(&p), true) {
            Ok((id, new)) => out.push(json!({"path": p, "id": id, "new": new})),
            Err(e) => out.push(json!({"path": p, "error": e.to_string()})),
        }
    }
    Ok(Value::Array(out))
}

#[tauri::command]
async fn docs_sync(st: State<'_, AppState>) -> R<Value> {
    let (n, errors) = documents::sync(&st.db, &st.paths).map_err(err)?;
    Ok(json!({"added": n, "errors": errors}))
}

#[tauri::command]
fn doc_facts(st: State<AppState>, id: i64) -> R<Value> {
    serde_json::to_value(documents::facts(&st.db, id).map_err(err)?).map_err(err)
}

#[tauri::command]
fn doc_text(st: State<AppState>, id: i64) -> R<Option<String>> {
    documents::text(&st.db, id).map_err(err)
}

#[tauri::command]
fn fact_review(st: State<AppState>, id: i64, status: String, corrected: Option<String>) -> R<()> {
    documents::review_fact(&st.db, id, &status, corrected.as_deref()).map_err(err)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn fact_add(st: State<AppState>, document_id: i64, key: String, label: String, value: String, page: Option<i64>, context: String) -> R<i64> {
    documents::add_manual_fact(&st.db, document_id, &key, &label, &value, page, &context).map_err(err)
}

#[tauri::command]
fn doc_meta(st: State<AppState>, id: i64, note: String, tags: String) -> R<()> {
    documents::update_meta(&st.db, id, &note, &tags).map_err(err)
}

#[tauri::command]
async fn doc_reanalyze(st: State<'_, AppState>, id: i64) -> R<usize> {
    documents::reanalyze(&st.db, id).map_err(err)
}

#[tauri::command]
fn docs_search(st: State<AppState>, q: String) -> R<Value> {
    serde_json::to_value(documents::search(&st.db, &q).map_err(err)?).map_err(err)
}

#[tauri::command]
fn open_path(path: String) -> R<()> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(err)
}

#[tauri::command]
fn starter_status(st: State<AppState>) -> R<Value> {
    let mut v = st.starter.lock().unwrap().clone();
    v["installed_at"] = json!(smi_data::starter::installed(&st.db));
    v["root"] = json!(st.paths.root);
    Ok(v)
}

/// Premier lancement : installe le pack de départ livré dans les ressources de l'application
/// (`starter/`), dans un thread, sans bloquer l'ouverture de la fenêtre.
fn install_starter_in_background(state: &AppState, pack: Option<std::path::PathBuf>) {
    if smi_data::starter::installed(&state.db).is_some() {
        return;
    }
    let Some(pack) = pack.filter(|p| p.is_dir()) else {
        *state.starter.lock().unwrap() = json!({"status": "absent", "message": "aucun pack de départ dans cette installation"});
        return;
    };
    *state.starter.lock().unwrap() = json!({"status": "running", "message": "Installation des données de départ…"});
    let (db, paths, status, s) = (state.db.clone(), state.paths.clone(), state.starter.clone(), state.settings());
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| smi_data::starter::install(&db, &paths, &s, Some(&pack))));
        *status.lock().unwrap() = match r {
            Ok(Ok(rep)) => json!({"status": "done", "report": rep}),
            Ok(Err(e)) => json!({"status": "error", "message": format!("{e:#}")}),
            Err(_) => json!({"status": "error", "message": "erreur interne pendant l'installation du pack"}),
        };
    });
}

macro_rules! handlers {
    () => {
        tauri::generate_handler![
            app_info,
            structure_check,
            dashboard,
            alert_resolve,
            settings_get,
            settings_save,
            collector_enable,
            job_status,
            job_control,
            jobs_list,
            market_query,
            market_ranking,
            mode_full_stats,
            import_slotlab,
            import_lut,
            collect_start,
            generator_peers,
            generate_preview,
            model_save,
            models_list,
            model_get,
            model_diff,
            model_set_status,
            sim_start,
            sessions_start,
            simulations_list,
            simulation_get,
            simulations_compare,
            validate_start,
            prompt_build,
            export_files,
            reels_demo,
            reels_exact,
            docs_list,
            docs_import,
            docs_sync,
            doc_facts,
            doc_text,
            fact_review,
            fact_add,
            doc_meta,
            doc_reanalyze,
            docs_search,
            open_path,
            starter_status,
        ]
    };
}

pub fn new_state(paths: Paths) -> AppState {
    let _ = paths.ensure();
    let db = Arc::new(Db::open(&paths.database).expect("ouverture de la base SQLite"));
    AppState { paths, db, jobs: Mutex::new(HashMap::new()), next: AtomicU64::new(0), starter: Arc::new(Mutex::new(json!({"status": "absent"}))) }
}

pub fn run() {
    let state = new_state(Paths::detect());
    let (paths, db) = (state.paths.clone(), state.db.clone());
    db.log("application", "Démarrage");
    // Actualisation automatique de la collecte (seulement si l'utilisateur l'a activée).
    {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(600));
            let s = AppSettings::load(&db);
            let Some(h) = s.collector.auto_refresh_hours.filter(|_| s.collector.enabled) else { continue };
            let last: Option<String> = db
                .conn()
                .query_row("SELECT c.started_at FROM collections c JOIN sources s ON s.id = c.source_id WHERE s.kind = 'api' AND c.status = 'OK' ORDER BY c.id DESC LIMIT 1", [], |r| r.get(0))
                .ok();
            let due = last.and_then(|t| chrono_hours_since(&t)).is_none_or(|age| age >= h as f64);
            if due {
                let _ = collector::collect(&db, &paths, &s, &|_, _| {}, &std::sync::atomic::AtomicBool::new(false));
            }
        });
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(|app| {
            use tauri::Manager;
            let pack = app.path().resource_dir().ok().map(|d| d.join("starter"));
            install_starter_in_background(&app.state::<AppState>(), pack);
            Ok(())
        })
        .invoke_handler(handlers!())
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Slot Math Intelligence");
}

/// Heures écoulées depuis une date « AAAA-MM-JJ HH:MM:SS » (heure locale), sans dépendance de date.
fn chrono_hours_since(t: &str) -> Option<f64> {
    let now = smi_data::now();
    let parse = |s: &str| -> Option<f64> {
        let (d, h) = s.split_once(' ')?;
        let p: Vec<f64> = d.split('-').chain(h.split(':')).map(|x| x.parse().ok()).collect::<Option<Vec<_>>>()?;
        // Jours approximés depuis l'an 0 (suffisant pour un écart de quelques heures ou jours).
        Some(((p[0] * 365.25 + p[1] * 30.44 + p[2]) * 24.0) + p[3] + p[4] / 60.0)
    };
    Some(parse(&now)? - parse(t)?)
}

#[cfg(test)]
mod ipc_tests;
