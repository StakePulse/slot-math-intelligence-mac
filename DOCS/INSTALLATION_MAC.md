# Installation sur Mac

macOS 11 (Big Sur) ou plus récent, Mac Apple Silicon (M1 à M4) ou Intel : l'application est
**universelle**, un seul fichier pour les deux.

## 1. Obtenir le fichier `.dmg`

Deux façons de le fabriquer (on ne peut pas fabriquer une application Mac depuis Windows) :

- **Sans Mac, par GitHub** : le dépôt contient `.github/workflows/macos.yml`. Sur github.com, onglet
  **Actions** → **macOS** → **Run workflow**, choisir l'édition (**partage** par défaut, ou **complet**).
  20 à 25 minutes plus tard (plus l'attente d'une machine libre), l'archive
  `Slot-Math-Intelligence-macOS-<édition>` (en bas de la page de l'exécution) contient le `.dmg` et `smi`.
  Cette chaîne lance aussi tous les tests sur macOS et simule un premier lancement.
- **Sur un Mac** : voir « Compiler sur un Mac » plus bas (`bash construire-mac.sh`).

Deux éditions, même application :
- **partage** (pour d'autres personnes) : référence datée du marché + modèle d'exemple + guides ;
- **complet** (pour l'auteur) : en plus les tables de FINDERS KEEPERS, les documents et les fichiers du projet.

## 2. Installer

1. Ouvrir `Slot Math Intelligence_0.3.0_universal.dmg`, glisser l'application dans **Applications**.
2. **Premier lancement** : l'application n'est pas signée par un compte développeur Apple (signature
   « ad hoc »), macOS la bloque donc la première fois.
   - macOS 15 (Sequoia) et plus : double-cliquer l'application, fermer l'avertissement, puis
     **Réglages Système → Confidentialité et sécurité** → en bas, **Ouvrir quand même** → confirmer.
   - macOS 11 à 14 : clic droit (ou Ctrl-clic) sur l'application → **Ouvrir** → **Ouvrir**.
   - Si macOS dit que l'application « est endommagée » (fichier téléchargé), lancer une fois dans le
     Terminal : `xattr -cr "/Applications/Slot Math Intelligence.app"`, puis rouvrir.
3. macOS demande l'accès au dossier **Documents** : accepter (c'est là que vivent la base et les fichiers).

Les lancements suivants se font normalement (Launchpad, Spotlight, Dock).

## 3. Où sont les données

`~/Documents/SLOT-MATH-INTELLIGENCE/` (dossier Documents de votre compte), même arborescence que sur Windows :

| Dossier | Contenu |
|---|---|
| `DATABASE/` | `smi.sqlite`, `imports/` (sources datées), `luts/` |
| `DOCUMENTS/` | `PDF/`, `SCREENSHOTS/`, `EXCEL/`, `OTHER/`, `INBOX/` (déposer ici, puis « Intégrer ») |
| `EXPORTS/` | `prompts/`, `reports/`, `models/`, `simulations/` |
| `DOCS/` | ce guide et le guide d'utilisation |
| `PROJECT/` | édition complète seulement : `MASTER_PROMPT.md`, `AUDIT.md`, `DECISIONS.md`, `ROADMAP.md` |

**Premier lancement** : le Dashboard affiche « installation des données de départ » pendant
quelques secondes. L'application copie son pack (sans jamais écraser un fichier existant) et
l'importe : instantané du marché du 2026-11-03 (125 slots, 1 116 modes) et le modèle d'exemple « ×5 000 »
régénéré par le moteur (statut DRAFT : le valider dans Math Engine). L'édition complète ajoute les 5 tables
de FINDERS KEEPERS analysées exactement et les 10 documents.

La marche à suivre pour concevoir une math propre avec l'outil est la même que sous Windows : voir
« Des maths propres en 5 étapes » dans `INSTALLATION_WINDOWS.md`.

Pour utiliser un autre dossier : lancer l'application avec la variable `SMI_ROOT` (voir plus bas).
Les simulations et validations faites sous Windows ne sont pas transférées : la base est propre à
chaque machine.

## Extraction des PDF (facultatif)

Sans rien installer, les PDF sont lus par l'extracteur Rust intégré. Pour une meilleure mise en page
et le découpage en pages, installer Poppler avec Homebrew : `brew install poppler`. L'application
trouve `pdftotext` dans `/opt/homebrew/bin` (Apple Silicon) ou `/usr/local/bin` (Intel) ; bouton
« Réanalyser » dans Document Library pour les documents déjà intégrés.

## Outil en ligne de commande `smi`

Fourni à côté du `.dmg`. Dans le Terminal :

```bash
chmod +x smi && xattr -c smi
./smi help
```

Il travaille sur le même dossier `~/Documents/SLOT-MATH-INTELLIGENCE` (ou sur `SMI_ROOT`).

## Compiler sur un Mac (développeur)

Prérequis : outils de développement d'Apple (`xcode-select --install`), Rust stable
(https://rustup.rs), Node.js 20 ou plus (https://nodejs.org).

```bash
bash construire-mac.sh
```

Le script vérifie les prérequis, construit l'interface, lance tous les tests, fabrique l'application
universelle et dépose le `.dmg` et `smi` dans `LIVRAISON/`.

Développement avec rechargement de l'interface : `cd SOURCE/app && npm ci && npx tauri dev`
(données dans le dossier du dépôt, car l'exécutable y trouve `PROJECT/MASTER_PROMPT.md`).

## Tests

```bash
cd SOURCE
cargo test --release -p smi-core -p smi-data
cargo test -p slot-math-intelligence
```

Mêmes tests que la version Windows, plus le pack de départ : installation sur un dossier vide, puis
seconde installation sans aucun doublon (`smi-data`), et premier lancement en arrière-plan suivi par
l'interface (`slot-math-intelligence`).
