# Slot Math Intelligence — code portable (Windows + macOS)

Laboratoire mathématique local pour concevoir des slots : référence du marché, moteur mathématique exact
en Rust, simulateur multithread reproductible, générateur de lois, contrôle qualité en 9 étapes et prompts
complets pour Claude. Ce dossier est né pour la version Mac ; son code est le même pour Windows et macOS
et produit aussi l'**édition à partager**.

| Dossier | Contenu |
|---|---|
| `PROJECT/` | `MASTER_PROMPT.md`, `AUDIT.md`, `DECISIONS.md` (D-15 macOS, D-16 éditions), `ROADMAP.md` |
| `DOCS/` | `INSTALLATION_WINDOWS.md`, `INSTALLATION_MAC.md`, `GUIDE_UTILISATEUR.md`, `MISES_A_JOUR.md` |
| `SOURCE/` | workspace Rust (`crates/`), application Tauri/React (`app/`), `packs/` (données de départ) |
| `.github/workflows/macos.yml` | fabrication du `.dmg` sur une machine macOS de GitHub, tests compris |
| `construire-mac.sh` | la même fabrication, sur un Mac |
| `LIVRAISON/` | fichiers prêts à envoyer (non suivi par Git) |

## Deux éditions, même application

| | à partager (par défaut) | complète |
|---|---|---|
| Référence du marché (instantané daté du 2026-11-03, 125 slots) | oui | oui |
| Modèle d'exemple ×5 000 | oui | oui |
| Guides d'utilisation et d'installation | oui | oui |
| Tables de FINDERS KEEPERS, documents (PDF, captures), `PROJECT/` | **non** | oui |
| Configuration | `tauri.conf.json` | + `tauri.complet.conf.json` |

Un test (`shareable_edition_config_has_no_private_data`) et l'action macOS vérifient que l'édition à
partager n'embarque rien de privé.

## Fabriquer

- **Windows** (sur ce PC) : `cd SOURCE/app && npx tauri build --bundles nsis`
  → `SOURCE/target/release/bundle/nsis/Slot Math Intelligence_<version>_x64-setup.exe`
  (édition complète : ajouter `--config src-tauri/tauri.complet.conf.json`).
- **macOS** : action GitHub **macOS** (« Run workflow », choix de l'édition) ou `bash construire-mac.sh [complet]` sur un Mac.

Données de l'application installée : `Documents/SLOT-MATH-INTELLIGENCE` (Windows : `Bureau\SLOT-MATH-INTELLIGENCE`
reste prioritaire s'il existe déjà). Au premier lancement, le pack de départ est copié sans rien écraser puis importé.
