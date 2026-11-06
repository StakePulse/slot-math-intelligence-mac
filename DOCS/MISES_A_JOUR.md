# Mises à jour

## Mettre à jour l'application
1. Modifier le code dans `SOURCE/`, augmenter `version` dans `SOURCE/Cargo.toml` (`[workspace.package]`),
   `SOURCE/app/package.json` et `SOURCE/app/src-tauri/tauri.conf.json`.
2. `cargo test --release -p smi-core -p smi-data` puis `cargo test -p slot-math-intelligence` : tout doit passer,
   en particulier la non-régression FINDERS KEEPERS et le pack de départ.
3. Fabriquer le `.dmg` : `bash construire-mac.sh` sur un Mac, ou l'action GitHub **macOS** (« Run workflow »,
   ou pousser un tag `v0.1.1`).
4. Ouvrir le nouveau `.dmg` et glisser l'application dans **Applications** en remplaçant l'ancienne.
   Le dossier `~/Documents/SLOT-MATH-INTELLIGENCE` (base, documents, exports) n'est pas touché.

Le pack de départ ne s'installe qu'une fois par machine (réglage `starter_installed`) : une mise à jour
n'écrase jamais vos données. Pour ajouter après coup des données livrées avec une nouvelle version :
les importer depuis l'interface, ou `smi init <dossier-du-pack>` (copie sans écrasement, imports dédoublonnés).

## Faire évoluer la base
Le schéma est versionné (`PRAGMA user_version`). Ajouter une migration = ajouter une chaîne SQL À LA FIN
de `MIGRATIONS` dans `SOURCE/crates/smi-data/src/db.rs` ; ne jamais modifier une migration existante.
Les bases existantes sont migrées à l'ouverture.

## Ajouter une source de données (nouveau provider, nouveau site)
Écrire un module à côté de `collector.rs` qui produit : une `collection` (source, date, statut),
des `slots` / `popularity` (bets et turnover séparés) et, si une lookup table est disponible,
`market::store_exact_mode` (statistiques EXACTES par notre moteur). Une donnée absente reste NULL
(« non disponible »), jamais une valeur par défaut. Si la source interdit l'accès automatisé, le module
doit être désactivé par défaut comme la collecte StakeCruncher (DECISIONS.md D-03).

## Ajouter des documents
Les déposer dans `DOCUMENTS/INBOX` depuis le Finder (ou n'importe où sous `DOCUMENTS/`) puis « Intégrer le
dossier DOCUMENTS » dans Document Library, ou `./smi docs`. Dédoublonnage par SHA-256 : réintégrer ne crée
pas de doublon.

## Modifier un seuil
Settings → Seuils et tolérances (volatilité, tolérances de validation, catégories, barèmes Stake).
Noter tout changement de valeur par défaut dans `PROJECT/DECISIONS.md`.
