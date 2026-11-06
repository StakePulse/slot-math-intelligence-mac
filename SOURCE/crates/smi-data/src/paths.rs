//! Emplacement du dossier de données. Ordre de recherche :
//! 1. variable d'environnement `SMI_ROOT` ;
//! 2. un dossier parent de l'exécutable qui contient `PROJECT/MASTER_PROMPT.md` (lancement depuis les sources) ;
//! 3. macOS (et Linux) : `~/Documents/SLOT-MATH-INTELLIGENCE` (créé au premier lancement, rempli par le
//!    pack de départ ; dossier visible dans le Finder pour y déposer des documents) ;
//!    Windows : `Bureau\SLOT-MATH-INTELLIGENCE` s'il existe déjà, sinon `Documents\SLOT-MATH-INTELLIGENCE`
//!    (voir [`Paths::default_root`]).
//!
//! Dans un paquet `.app`, l'exécutable est dans `Slot Math Intelligence.app/Contents/MacOS/` : l'étape 2 ne
//! trouve rien et l'on passe à l'étape 3. Le dossier du Bureau n'est pas sondé sur macOS : y accéder
//! déclencherait une demande d'autorisation supplémentaire du système.

use serde::Serialize;
use std::path::{Path, PathBuf};

pub const ROOT_NAME: &str = "SLOT-MATH-INTELLIGENCE";

#[derive(Clone, Debug, Serialize)]
pub struct Paths {
    pub root: PathBuf,
    pub database: PathBuf,
    pub imports: PathBuf,
    pub luts: PathBuf,
    pub documents: PathBuf,
    pub exports: PathBuf,
    pub project: PathBuf,
}

pub const DOC_FOLDERS: [&str; 5] = ["PDF", "SCREENSHOTS", "EXCEL", "OTHER", "INBOX"];
pub const EXPORT_FOLDERS: [&str; 4] = ["prompts", "reports", "models", "simulations"];

impl Paths {
    pub fn at(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            database: root.join("DATABASE").join("smi.sqlite"),
            imports: root.join("DATABASE").join("imports"),
            luts: root.join("DATABASE").join("luts"),
            documents: root.join("DOCUMENTS"),
            exports: root.join("EXPORTS"),
            project: root.join("PROJECT"),
        }
    }

    pub fn detect() -> Self {
        if let Ok(r) = std::env::var("SMI_ROOT") {
            return Self::at(Path::new(&r));
        }
        if let Ok(exe) = std::env::current_exe() {
            let mut p = exe.parent().map(Path::to_path_buf);
            while let Some(dir) = p {
                if dir.join("PROJECT").join("MASTER_PROMPT.md").exists() {
                    return Self::at(&dir);
                }
                p = dir.parent().map(Path::to_path_buf);
            }
        }
        Self::at(&Self::default_root())
    }

    /// Dossier de données par défaut de la plateforme (étape 3).
    ///
    /// Windows : un dossier `SLOT-MATH-INTELLIGENCE` déjà posé sur le Bureau reste prioritaire (installation
    /// d'origine de l'auteur) ; sinon le VRAI dossier Documents de l'utilisateur (dossier connu de Windows,
    /// donc aussi quand il est redirigé vers OneDrive). Dernier recours : `%APPDATA%\SlotMathIntelligence`.
    #[cfg(windows)]
    pub fn default_root() -> PathBuf {
        if let Some(desk) = dirs::desktop_dir() {
            let d = desk.join(ROOT_NAME);
            if d.exists() {
                return d;
            }
        }
        if let Some(docs) = dirs::document_dir() {
            return docs.join(ROOT_NAME);
        }
        let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
        base.join("SlotMathIntelligence")
    }

    /// Dossier de données par défaut de la plateforme (étape 3).
    #[cfg(not(windows))]
    pub fn default_root() -> PathBuf {
        if let Some(docs) = dirs::document_dir() {
            return docs.join(ROOT_NAME);
        }
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        home.join("Documents").join(ROOT_NAME)
    }

    /// Crée l'arborescence manquante et renvoie la liste des dossiers vérifiés.
    pub fn ensure(&self) -> std::io::Result<Vec<(String, bool)>> {
        let mut dirs = vec![
            self.root.join("PROJECT"),
            self.root.join("DATABASE"),
            self.imports.clone(),
            self.luts.clone(),
            self.documents.clone(),
            self.exports.clone(),
        ];
        dirs.extend(DOC_FOLDERS.iter().map(|d| self.documents.join(d)));
        dirs.extend(EXPORT_FOLDERS.iter().map(|d| self.exports.join(d)));
        let mut out = Vec::new();
        for d in dirs {
            let existed = d.exists();
            std::fs::create_dir_all(&d)?;
            out.push((d.display().to_string(), existed));
        }
        Ok(out)
    }
}
