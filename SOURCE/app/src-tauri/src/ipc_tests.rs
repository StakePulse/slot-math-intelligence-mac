//! Test d'intégration de l'IPC : chaque commande est appelée comme le fait l'interface (mêmes noms
//! d'arguments en camelCase, mêmes formes JSON), sur une base temporaire, avec le runtime simulé de
//! Tauri (aucune fenêtre ouverte). Les données réelles du projet sont lues, jamais modifiées.

use super::*;
use serde_json::json;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::WebviewWindowBuilder;

struct Ctx {
    _app: tauri::App<tauri::test::MockRuntime>,
    webview: tauri::WebviewWindow<tauri::test::MockRuntime>,
}

impl Ctx {
    fn call(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        let req = InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            // Origine locale de l'application : `http://tauri.localhost` sous Windows (et Android),
            // `tauri://localhost` sur macOS et Linux. Une autre origine est traitée comme distante et
            // refusée par les autorisations (« not allowed »). Même règle que l'exemple de `tauri::test`.
            url: if cfg!(any(windows, target_os = "android")) { "http://tauri.localhost" } else { "tauri://localhost" }.parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        get_ipc_response(&self.webview, req).map(|b| b.deserialize::<Value>().unwrap())
    }

    fn ok(&self, cmd: &str, args: Value) -> Value {
        match self.call(cmd, args.clone()) {
            Ok(v) => v,
            Err(e) => panic!("{cmd} {args} → erreur {e}"),
        }
    }

    fn wait(&self, id: &Value) -> Value {
        for _ in 0..1200 {
            let j = self.ok("job_status", json!({"id": id}));
            let s = j["status"].as_str().unwrap().to_string();
            if s == "done" || s == "error" || s == "cancelled" {
                assert_eq!(s, "done", "tâche {}", j);
                return j["result"].clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("tâche trop longue");
    }
}

/// Données réelles livrées avec l'application (`packs/commun`, `packs/prive`) : lues, jamais modifiées.
fn pack(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packs").join(name)
}

fn setup() -> Ctx {
    let tmp = std::env::temp_dir().join(format!("smi-ipc-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let state = new_state(Paths::at(&tmp));
    let app = mock_builder().manage(state).invoke_handler(handlers!()).build(mock_context(noop_assets())).unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    Ctx { _app: app, webview }
}

#[test]
fn every_screen_call_works() {
    let c = setup();

    // Application et réglages
    assert!(c.ok("app_info", json!({}))["engine"].is_string());
    let s = c.ok("settings_get", json!({}));
    let mut settings = s["settings"].clone();
    settings["performance_mode"] = json!("eco");
    c.ok("settings_save", json!({"settings": settings}));
    assert!(c.ok("structure_check", json!({}))["dirs"].is_array());

    // Collecte : refusée tant qu'elle n'est pas activée
    assert!(c.call("collect_start", json!({})).is_err());
    c.ok("collector_enable", json!({"enabled": false}));

    // Marché : instantané tiers + table locale
    let snap = pack("commun").join("DATABASE/imports/slotlab_market_2026-11-03.json");
    let fk = pack("prive").join("DATABASE/imports/our_games/finders-keepers_2026-11-05/lookUpTable_base_0.csv");
    assert!(snap.exists() && fk.exists(), "packs de départ introuvables");
    {
        let r = c.ok("import_slotlab", json!({"path": snap}));
        assert_eq!(r["modes"], 1116);
    }
    {
        c.ok("import_lut", json!({"path": fk, "slotName": "Finders Keepers", "provider": "Oracle Labs", "mode": "base", "cost": 1.0, "ours": true}));
    }
    let filter = json!({"providers": [], "kinds": ["base"], "vol_classes": [], "max_min": null, "max_max": null, "rtp_min": null, "rtp_max": null, "top_n": 50, "include_ours": false, "search": null});
    let m = c.ok("market_query", json!({"filter": filter}));
    assert!(m["aggregates"].as_array().unwrap().len() == 9);
    c.ok("market_ranking", json!({}));
    let all = c.ok("market_query", json!({"filter": {"providers": [], "kinds": [], "vol_classes": [], "include_ours": true}}));
    if let Some(ours) = all["rows"].as_array().unwrap().iter().find(|r| r["ours"] == json!(true)) {
        let full = c.ok("mode_full_stats", json!({"modeId": ours["mode_id"]}));
        assert!((full["stats"]["rtp"].as_f64().unwrap() - 0.965).abs() < 1e-12);
        let sim = c.ok("sim_start", json!({"target": {"kind": "mode", "mode_id": ours["mode_id"]}, "spins": 500000, "seed": 1, "threads": null}));
        let r = c.wait(&sim);
        assert_eq!(r["kind"], "mode");
    }
    c.ok("generator_peers", json!({"maxWin": 5000.0, "price": 1.0}));

    // Générateur → modèle → simulation → validation → prompt → exports
    let spec = json!({"name": "Test IPC", "game_type": "Video slot", "mechanics": ["Ways"], "price": 1, "rtp": 0.965, "max_win": 5000, "max_win_one_in": 600000,
                      "max_rtp_share": 0.015, "hit_rate": 0.26, "bonus_one_in": 170, "bonus_share": 0.4, "volatility": "Medium", "sd_target": null,
                      "base_max_win": null, "bonus_floor": null, "granularity": 0.1});
    let p = c.ok("generate_preview", json!({"spec": spec}));
    assert!((p["stats"]["rtp"].as_f64().unwrap() - 0.965).abs() < 1e-9);
    let saved = c.ok("model_save", json!({"spec": spec, "modelId": null, "note": "test"}));
    let id = saved["id"].clone();
    assert!(c.ok("models_list", json!({})).as_array().unwrap().len() == 1);
    let mg = c.ok("model_get", json!({"id": id, "version": null}));
    assert_eq!(mg["model"]["status"], "DRAFT");
    let sim = c.ok("sim_start", json!({"target": {"kind": "model", "id": id, "version": null}, "spins": 1000000, "seed": 3, "threads": null}));
    let r = c.wait(&sim);
    assert!(r["result"]["comparison"]["rtp_z"].as_f64().unwrap().abs() < 5.0);
    let v = c.ok("validate_start", json!({"id": id, "version": null, "spins": 1000000, "seed": 3}));
    let rep = c.wait(&v);
    assert_eq!(rep["status"], "VALIDATED", "{rep}");
    c.ok("model_set_status", json!({"id": id, "version": 1, "status": "READY_FOR_EXPORT"}));
    let pr = c.ok("prompt_build", json!({"id": id, "version": null}));
    assert!(pr["markdown"].as_str().unwrap().contains("Cahier des charges"));
    for what in ["prompt_md", "prompt_txt", "model_json", "validation"] {
        let e = c.ok("export_files", json!({"id": id, "version": null, "what": what}));
        for f in e["files"].as_array().unwrap() {
            assert!(std::path::Path::new(f.as_str().unwrap()).exists());
        }
    }
    let mut spec2 = spec.clone();
    spec2["hit_rate"] = json!(0.3);
    let s2 = c.ok("model_save", json!({"spec": spec2, "modelId": id, "note": "hit 30 %"}));
    assert_eq!(s2["version"], 2);
    let d = c.ok("model_diff", json!({"id": id, "a": 1, "b": 2}));
    assert!(d.as_array().unwrap().iter().any(|r| r["field"] == "Hit rate cible" && r["changed"] == json!(true)));

    // Sessions
    let ss = c.ok("sessions_start", json!({"target": {"kind": "model", "id": id, "version": 1}, "rounds": 100, "sessions": 3000, "seed": 1, "bankroll": 100.0, "bigWin": 50.0}));
    let sr = c.wait(&ss);
    assert!(sr["survival"].as_array().unwrap().len() > 3);

    // Moteur de règles
    let game = c.ok("reels_demo", json!({}));
    let ex = c.ok("reels_exact", json!({"game": game}));
    assert!(ex["trigger_prob"].as_f64().unwrap() > 0.0);
    let rs = c.ok("sim_start", json!({"target": {"kind": "reels", "game": game, "buy": null}, "spins": 100000, "seed": 2, "threads": null}));
    let rr = c.wait(&rs);
    assert_eq!(rr["kind"], "reels");

    // Comparaison de simulations
    let list = c.ok("simulations_list", json!({"modelId": null}));
    let ids: Vec<i64> = list.as_array().unwrap().iter().map(|s| s["id"].as_i64().unwrap()).take(3).collect();
    let cmp = c.ok("simulations_compare", json!({"ids": ids}));
    assert!(cmp["rows"].as_array().unwrap().len() >= 2);

    // Documents
    let pdf = pack("prive").join("DOCUMENTS/PDF/TempleDrop_Mathematical_Model_v1.pdf");
    assert!(pdf.exists());
    {
        let r = c.ok("docs_import", json!({"paths": [pdf]}));
        let doc = r[0]["id"].clone();
        let facts = c.ok("doc_facts", json!({"id": doc}));
        if let Some(f) = facts.as_array().unwrap().first() {
            c.ok("fact_review", json!({"id": f["id"], "status": "VALIDÉ", "corrected": null}));
        }
        c.ok("fact_add", json!({"documentId": doc, "key": "rtp", "label": "RTP", "value": "96,70", "page": 3, "context": "test"}));
        c.ok("doc_meta", json!({"id": doc, "note": "note", "tags": "modèle"}));
        assert!(c.ok("doc_text", json!({"id": doc})).as_str().unwrap_or("").contains("Temple"));
        c.ok("docs_search", json!({"q": "RTP"}));
    }
    c.ok("docs_sync", json!({}));
    assert!(c.ok("docs_list", json!({})).is_array());

    // Tableau de bord
    let dash = c.ok("dashboard", json!({}));
    assert!(dash["models"].as_i64().unwrap() >= 1);
    assert!(c.ok("jobs_list", json!({})).as_array().unwrap().len() >= 4);

    // Pack de départ : pas de pack dans ce runtime simulé → « absent », jamais une erreur.
    let st = c.ok("starter_status", json!({}));
    assert_eq!(st["status"], "absent");
    assert!(st["root"].is_string());
}

/// Premier lancement de l'édition à partager : le pack commun est installé en arrière-plan, l'état passe
/// à « done » ; marché et modèle d'exemple présents, aucune table de nos jeux, aucun document.
#[test]
fn first_launch_installs_starter_pack() {
    let tmp = std::env::temp_dir().join(format!("smi-first-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let state = new_state(Paths::at(&tmp));
    install_starter_in_background(&state, Some(pack("commun")));
    let app = mock_builder().manage(state).invoke_handler(handlers!()).build(mock_context(noop_assets())).unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let c = Ctx { _app: app, webview };
    let mut last = Value::Null;
    for _ in 0..1200 {
        last = c.ok("starter_status", json!({}));
        if last["status"] != "running" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(last["status"], "done", "{last}");
    assert!(last["installed_at"].is_string());
    assert_eq!(last["report"]["errors"].as_array().unwrap().len(), 0, "{last}");
    let dash = c.ok("dashboard", json!({}));
    assert_eq!(dash["slots"].as_i64().unwrap(), 125, "{dash}");
    assert_eq!(dash["models"].as_i64().unwrap(), 1);
    assert_eq!(dash["documents"].as_i64().unwrap(), 0);
}

/// Garde-fou : la configuration par défaut (édition à partager) n'embarque rien de privé ; seule la
/// surcouche `tauri.complet.conf.json` ajoute le pack privé et les documents du projet.
#[test]
fn shareable_edition_config_has_no_private_data() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let read = |f: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(dir.join(f)).unwrap()).unwrap() };
    let keys = |v: &Value| -> Vec<String> { v["bundle"]["resources"].as_object().unwrap().keys().cloned().collect() };
    let base = keys(&read("tauri.conf.json"));
    assert!(base.iter().any(|k| k.ends_with("packs/commun")), "{base:?}");
    for k in &base {
        assert!(!k.contains("prive") && !k.contains("PROJECT") && !k.contains("MISES_A_JOUR"), "ressource privée dans l'édition à partager : {k}");
    }
    let full = keys(&read("tauri.complet.conf.json"));
    assert!(full.iter().any(|k| k.ends_with("packs/prive")) && full.iter().any(|k| k.contains("PROJECT")), "{full:?}");
}

/// Contrôles de conception : un petit jeu publié (format Stake), analysé comme le fait l'onglet de Slot Explorer.
#[test]
fn design_checks_on_a_published_folder() {
    let c = setup();
    let dir = std::env::temp_dir().join(format!("smi-ipc-jeu-{}", std::process::id())).join("jeu-ipc").join("publish_files");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.json"), r#"{"modes":[{"name":"base","cost":1.0,"events":"books_base.jsonl","weights":"lookUpTable_base_0.csv"}]}"#).unwrap();
    std::fs::write(dir.join("lookUpTable_base_0.csv"), "1,900,0\n2,90,120\n3,10,2000\n").unwrap();
    std::fs::write(
        dir.join("books_base.jsonl"),
        [
            r#"{"id":1,"payoutMultiplier":0,"criteria":"basegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[["L1","L2"],["H1","S"]]}]}"#,
            r#"{"id":2,"payoutMultiplier":120,"criteria":"basegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[["L1","L1"],["L1","H2"]]},{"index":1,"type":"winInfo","wins":[{"symbol":"L1","amount":120}]}]}"#,
            r#"{"id":3,"payoutMultiplier":2000,"criteria":"freegame","events":[{"index":0,"type":"reveal","gameType":"basegame","board":[["S","L1"],["S","S"]]},{"index":1,"type":"freeSpinTrigger","positions":[[0,0],[1,0],[1,1]]},{"index":2,"type":"reveal","gameType":"freegame","board":[["H1"]]},{"index":3,"type":"winInfo","wins":[{"symbol":"H1","amount":2000}]}]}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    // Un dossier sans index.json est refusé avec une explication.
    assert!(c.call("design_analyze_start", json!({"path": dir.parent().unwrap()})).is_err());
    let before: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    let id = c.ok("design_analyze_start", json!({"path": dir}));
    let res = c.wait(&id);
    let rid = res["id"].clone();
    assert_eq!(res["report"]["game"], "jeu-ipc");
    let list = c.ok("design_reports_list", json!({}));
    assert_eq!(list[0]["id"], rid);
    let rep = c.ok("design_report_get", json!({"id": rid}));
    let keys: Vec<&str> = rep["modes"][0]["checks"].as_array().unwrap().iter().map(|k| k["key"].as_str().unwrap()).collect();
    for k in ["symboles_plateau", "connexions_hauts", "plancher", "ordre_scatters", "books_par_tranche", "doublons", "profil", "max_win", "part_bonus"] {
        assert!(keys.contains(&k), "contrôle {k} absent : {keys:?}");
    }
    let path = c.ok("design_report_export", json!({"id": rid}));
    assert!(std::fs::read_to_string(path.as_str().unwrap()).unwrap().contains("Contrôles de conception — jeu-ipc"));
    // Le dossier analysé est intact.
    let after: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(before, after);
}
