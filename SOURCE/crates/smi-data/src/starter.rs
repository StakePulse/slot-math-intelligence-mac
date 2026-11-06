//! Pack de départ : premier lancement sur une machine neuve (application installée sur macOS, dossier
//! de données vide). Les fichiers livrés avec l'application (documents du projet, instantané daté du
//! marché, tables de FINDERS KEEPERS, documents, spécification du modèle d'exemple) sont copiés dans
//! le dossier de données SANS JAMAIS écraser un fichier existant, puis importés comme le fait `smi init`.
//!
//! Chaque étape est idempotente : l'instantané est dédoublonné à l'import, les tables ne sont importées
//! que si le jeu n'a encore aucun mode, les documents sont dédoublonnés par SHA-256 et le modèle
//! d'exemple n'est créé que s'il n'existe pas déjà un modèle du même nom.

use crate::settings::AppSettings;
use crate::{documents, market, models, Db, Paths};
use rusqlite::params;
use serde::Serialize;
use smi_core::generator::{generate, GeneratorSpec};
use std::path::Path;

/// Réglage écrit une fois le pack installé (date d'installation).
pub const SETTING_KEY: &str = "starter_installed";

pub const SNAPSHOT: &str = "slotlab_market_2026-11-03.json";
pub const FK_DIR: &str = "finders-keepers_2026-11-05";
pub const FK_MODES: [(&str, f64); 5] = [("base", 1.0), ("nid", 100.0), ("grand_nid", 200.0), ("nid_or", 300.0), ("spin_tresor", 400.0)];
pub const EXAMPLE_SPEC: &str = "exemple-5000.spec.json";

#[derive(Debug, Default, Clone, Serialize)]
pub struct StarterReport {
    /// Fichiers copiés depuis le pack.
    pub copied: usize,
    /// Fichiers déjà présents, laissés tels quels.
    pub kept: usize,
    pub messages: Vec<String>,
    pub errors: Vec<String>,
}

/// Copie récursive sans écrasement. Renvoie (copiés, déjà présents).
pub fn copy_missing(src: &Path, dst: &Path) -> std::io::Result<(usize, usize)> {
    let (mut copied, mut kept) = (0, 0);
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            let (c, k) = copy_missing(&from, &to)?;
            copied += c;
            kept += k;
        } else if to.exists() {
            kept += 1;
        } else {
            std::fs::copy(&from, &to)?;
            copied += 1;
        }
    }
    Ok((copied, kept))
}

pub fn installed(db: &Db) -> Option<String> {
    db.setting_get(SETTING_KEY)
}

fn fk_modes_count(db: &Db) -> i64 {
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM modes m JOIN slots s ON s.id = m.slot_id WHERE s.slug = ?1",
            params![market::slugify("Oracle Labs-Finders Keepers")],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

fn model_named(db: &Db, name: &str) -> bool {
    db.conn().query_row("SELECT COUNT(*) FROM models WHERE name = ?1", params![name], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0
}

/// Importe les données de départ présentes dans le dossier du projet. Une étape qui échoue est notée
/// dans le rapport sans empêcher les suivantes.
pub fn seed(db: &Db, paths: &Paths, s: &AppSettings, rep: &mut StarterReport) {
    // 1. Instantané daté du marché (IMPORTÉ).
    let snap = paths.imports.join(SNAPSHOT);
    if snap.exists() {
        match market::import_slotlab_snapshot(db, s, &snap) {
            Ok(r) => rep.messages.push(r.message),
            Err(e) => rep.errors.push(format!("instantané du marché : {e:#}")),
        }
    }
    // 2. Tables publiées de FINDERS KEEPERS (statistiques EXACTES par le moteur).
    let fk = paths.imports.join("our_games").join(FK_DIR);
    if fk.exists() && fk_modes_count(db) == 0 {
        for (mode, cost) in FK_MODES {
            let f = fk.join(format!("lookUpTable_{mode}_0.csv"));
            if !f.exists() {
                continue;
            }
            match market::import_lut_file(db, paths, s, &f, "Finders Keepers", "Oracle Labs", mode, cost, true) {
                Ok(r) => rep.messages.push(r.message),
                Err(e) => rep.errors.push(format!("table {mode} : {e:#}")),
            }
        }
    }
    // 3. Documents (dédoublonnés par SHA-256).
    match documents::sync(db, paths) {
        Ok((n, errs)) => {
            rep.messages.push(format!("{n} document(s) intégré(s)"));
            rep.errors.extend(errs);
        }
        Err(e) => rep.errors.push(format!("documents : {e:#}")),
    }
    // 4. Modèle d'exemple, régénéré par le moteur depuis sa spécification.
    let spec_file = paths.imports.join("models").join(EXAMPLE_SPEC);
    if spec_file.exists() {
        let r = (|| -> anyhow::Result<Option<String>> {
            let text = std::fs::read_to_string(&spec_file)?;
            let spec: GeneratorSpec = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
            if model_named(db, &spec.name) {
                return Ok(None);
            }
            let g = generate(&spec, &s.volatility)?;
            let (id, v) = models::create_model(db, &g, &["Pack de départ".into()], "Modèle d'exemple livré avec l'application (même spécification que la v2 de la version Windows)")?;
            Ok(Some(format!("modèle d'exemple « {} » créé ({id} v{v}, statut DRAFT : à valider)", spec.name)))
        })();
        match r {
            Ok(Some(m)) => rep.messages.push(m),
            Ok(None) => {}
            Err(e) => rep.errors.push(format!("modèle d'exemple : {e:#}")),
        }
    }
}

/// Installe le pack : copie sans écrasement depuis `starter_dir` (s'il existe), imports, puis marque
/// l'installation. À n'appeler qu'une fois (voir [`installed`]) ; un second appel ne duplique rien.
pub fn install(db: &Db, paths: &Paths, s: &AppSettings, starter_dir: Option<&Path>) -> anyhow::Result<StarterReport> {
    let mut rep = StarterReport::default();
    paths.ensure()?;
    if let Some(dir) = starter_dir.filter(|d| d.is_dir()) {
        let (c, k) = copy_missing(dir, &paths.root)?;
        rep.copied = c;
        rep.kept = k;
    }
    seed(db, paths, s, &mut rep);
    db.setting_set(SETTING_KEY, &crate::now())?;
    db.log(
        "pack de départ",
        &format!("{} fichier(s) copié(s), {} déjà présent(s) ; {} étape(s) ; {} erreur(s)", rep.copied, rep.kept, rep.messages.len(), rep.errors.len()),
    );
    for e in &rep.errors {
        db.alert("WARN", "pack de départ", e);
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starter_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../starter")
    }

    #[test]
    fn copy_never_overwrites() {
        let tmp = std::env::temp_dir().join(format!("smi-copy-{}", std::process::id()));
        let (a, b) = (tmp.join("a"), tmp.join("b"));
        std::fs::create_dir_all(a.join("x")).unwrap();
        std::fs::write(a.join("x").join("f.txt"), "pack").unwrap();
        std::fs::write(a.join("g.txt"), "pack").unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("g.txt"), "utilisateur").unwrap();
        assert_eq!(copy_missing(&a, &b).unwrap(), (1, 1));
        assert_eq!(std::fs::read_to_string(b.join("g.txt")).unwrap(), "utilisateur");
        assert_eq!(copy_missing(&a, &b).unwrap(), (0, 2));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Le vrai pack livré : installation sur un dossier vide, puis une seconde fois sans doublon.
    #[test]
    fn installs_real_pack_once() {
        let dir = starter_dir();
        assert!(dir.join("DATABASE/imports").join(SNAPSHOT).exists(), "pack de départ introuvable : {}", dir.display());
        let tmp = std::env::temp_dir().join(format!("smi-starter-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let paths = Paths::at(&tmp);
        paths.ensure().unwrap();
        let db = Db::open(&paths.database).unwrap();
        let s = AppSettings::default();
        assert!(installed(&db).is_none());
        let rep = install(&db, &paths, &s, Some(&dir)).unwrap();
        assert!(rep.errors.is_empty(), "erreurs : {:?}", rep.errors);
        assert!(installed(&db).is_some());
        // PROJECT/ et DOCS/ sont ajoutés au pack par tauri.conf.json (depuis la racine du dépôt).
        assert!(dir.join("../../PROJECT/MASTER_PROMPT.md").exists());
        assert!(paths.imports.join("our_games").join(FK_DIR).join("reference_figures.json").exists());
        let count = |sql: &str| -> i64 { db.conn().query_row(sql, [], |r| r.get(0)).unwrap() };
        let (slots, modes, docs, mods) = (count("SELECT COUNT(*) FROM slots"), count("SELECT COUNT(*) FROM modes"), count("SELECT COUNT(*) FROM documents"), count("SELECT COUNT(*) FROM models"));
        assert_eq!(fk_modes_count(&db), 5);
        assert!(slots >= 126, "slots : {slots}");
        assert!(docs >= 10, "documents : {docs}");
        assert_eq!(mods, 1);
        // Second passage : rien de dupliqué.
        let rep2 = install(&db, &paths, &s, Some(&dir)).unwrap();
        assert_eq!(rep2.copied, 0);
        assert_eq!(count("SELECT COUNT(*) FROM slots"), slots);
        assert_eq!(count("SELECT COUNT(*) FROM modes"), modes);
        assert_eq!(count("SELECT COUNT(*) FROM documents"), docs);
        assert_eq!(count("SELECT COUNT(*) FROM models"), 1);
        drop(db);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
