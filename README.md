# Slot Math Intelligence — version macOS

La même application que la version Windows (`Desktop/SLOT-MATH-INTELLIGENCE`), pour Mac : référence
du marché, moteur mathématique exact en Rust, simulateur multithread reproductible, générateur de lois,
contrôle qualité en 9 étapes et prompts complets pour Claude. Mêmes moteurs, mêmes écrans, mêmes tests.

| Dossier | Contenu |
|---|---|
| `PROJECT/` | `MASTER_PROMPT.md`, `AUDIT.md`, `DECISIONS.md` (D-15 : version macOS), `ROADMAP.md` |
| `DOCS/` | `INSTALLATION_MAC.md`, `GUIDE_UTILISATEUR.md`, `MISES_A_JOUR.md` |
| `SOURCE/` | workspace Rust (`crates/`), application Tauri/React (`app/`), `starter/` (pack de départ embarqué) |
| `.github/workflows/macos.yml` | fabrication du `.dmg` sur une machine macOS de GitHub, tests compris |
| `construire-mac.sh` | la même fabrication, sur un Mac |

Ce qui change par rapport à Windows :
- données dans `~/Documents/SLOT-MATH-INTELLIGENCE` ;
- au premier lancement, le pack de départ embarqué (marché daté, tables de FINDERS KEEPERS, documents,
  modèle d'exemple) est copié puis importé ;
- application universelle (Apple Silicon + Intel), `.dmg` ; `pdftotext` de Homebrew utilisé s'il est installé.

Démarrage : `DOCS/INSTALLATION_MAC.md`, puis `DOCS/GUIDE_UTILISATEUR.md`.
