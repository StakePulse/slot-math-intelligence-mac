# Décisions techniques (journal)

Chaque décision : contexte, choix, conséquence. Les seuils cités sont modifiables dans **Settings** ;
toute modification d'un seuil par défaut doit être notée ici.

## D-01 — Architecture : Tauri 2 + Rust + React/TypeScript + SQLite (2026-11-06)
Conforme au cahier des charges. Workspace Rust `SOURCE/` :
- `crates/smi-core` — moteur mathématique pur (aucune E/S) : lois exactes, statistiques, validation,
  générateur, simulateur, moteur de règles, agrégats de marché, prompt. Testé seul (`cargo test -p smi-core`).
- `crates/smi-data` — SQLite (rusqlite, SQLite embarqué), imports, collecte, documents, exports.
- `crates/smi-cli` — `smi.exe`, mêmes moteurs sans interface (scripts, contrôles, CI).
- `app/` — interface React + TypeScript (Vite, Recharts) ; `app/src-tauri` — commandes Tauri, sans calcul.
Python n'est utilisé pour AUCUN calcul (règle n°1). Versions Tauri reprises d'un projet qui compile déjà
sur ce poste (Patisserie : tauri 2.12, plugins dialog 2.8 / opener 2.7).

## D-02 — Une loi = une lookup table (2026-11-06)
Le moteur raisonne sur la forme publiée par Stake Engine : issues (paiement en centièmes de mise de
base, poids entier, classe de résultat). Toute statistique est une somme pondérée finie, calculée en
entiers (`u128`) puis convertie : EXACT. C'est la méthode des modèles Witchcraft et Temple Drop
(« books define what can happen; weights define how often »). Les classes de résultat (pas de gain /
base / bonus / max win) forment une partition ; une lookup table publique n'en a que deux (pas de
gain / gain) car elle ne dit pas quel round est un bonus.

## D-03 — Collecte StakeCruncher désactivée par défaut (2026-11-06)
Constat (AUDIT.md §3) : les pages publiques ne contiennent aucune donnée sans JavaScript ; les données
viennent de `/tracker-api/`, que `robots.txt` interdit aux robots. Choix : le collecteur est codé
(classement, catalogue, lookup tables ; débit ≥ 1 s, arrêt sur 429, cache, User-Agent identifiable,
contrôle de structure) mais **désactivé** ; l'utilisateur l'active dans Settings après avoir lu
l'avertissement (date d'acceptation enregistrée). Remplacements clairement identifiés : instantané tiers
daté (slotlab, 2026-11-03, nature IMPORTÉ) ; import de lookup tables (`id,weight,payout`) analysées
EXACTEMENT ; import CSV/JSON. Aucune donnée inventée : un champ absent est « non disponible ».

**Décision de l'utilisateur (2026-11-06) : garder la collecte désactivée.** Le marché vient des imports
(instantanés tiers datés, lookup tables, CSV/JSON). Ne pas activer le collecteur ni le proposer de nouveau
sans demande explicite ; pistes possibles si le besoin revient : accès autorisé par l'éditeur du site,
exports manuels.

## D-04 — Popularité = bets sur 30 jours ; turnover à part (2026-11-06)
Classement de référence au nombre de bets (30 j). Le turnover (et sa part) est affiché à côté, jamais
fusionné. Quand la source ne fournit pas les bets (instantané slotlab), l'application le dit et classe
au turnover. Les unités brutes du turnover de l'API ne sont pas documentées : on garde la valeur brute
et la part du total, sans conversion inventée.

## D-05 — Statistiques de marché (2026-11-06)
Jamais une moyenne seule : n, moyenne, médiane, moyennes pondérées (bets, turnover) quand le poids
existe, écart-type (n−1), min/max, P5/P10/Q1/Q3/P90/P95 (interpolation linéaire, type 7), extrêmes de
Tukey (1,5 × IQR), drapeau « échantillon insuffisant » sous `min_sample` (10). Un mode = une valeur.

## D-06 — Nature de chaque chiffre (2026-11-06)
EXACT · EXACT FORMULA · MONTE CARLO · CONFIGURATION · SPECIFICATION · IMPORTÉ — affichée sur chaque
carte et dans le prompt. Théorie et simulation ne sont jamais mélangées (règle n°5).

## D-07 — Classification de volatilité (2026-11-06)
Score composite ∈ [0, 1], moyenne pondérée de 5 indicateurs ramenés sur [0, 1] entre des ancres FIXES
(reproductible d'une collecte à l'autre) :

| indicateur | poids | ancres |
|---|---|---|
| σ / prix (log) | 0,40 | 3 → 100 |
| fréquence des pertes P(X=0) | 0,15 | 0,50 → 0,97 |
| part du RTP venue des gains ≥ 100× prix | 0,15 | 0,10 → 0,85 |
| part du retour portée par le top 1 % des rounds | 0,20 | 0,40 → 0,98 |
| P(gain ≥ 1 000× prix) (log, inverse) | 0,10 | 1e-3 → 1e-7 |

Coupures : < 0,25 Low · < 0,40 Medium-Low · < 0,55 Medium · < 0,70 Medium-High · < 0,85 High · Very High.
Plages σ/prix visées par le générateur : Low < 8 · 8–15 · 15–25 · 25–40 · 40–60 · Very High ≥ 60.
Calibration : 133 modes de base de l'instantané du 2026-11-03 (σ/prix médian 36, quartiles 29–47).

## D-08 — Catégories de gains (2026-11-06)
En multiples du PRIX : petits < 2× ; moyens 2–20× ; gros 20–500× ; très gros ≥ 500× (hors max) ; max.

## D-09 — Validation (2026-11-06)
9 étapes (Probabilités, RTP, Max win, Bonus, Distributions, Fréquences, Simulation, Théorie/simulation,
Rapport). Statut : un contrôle OBLIGATOIRE en échec → VALIDATION FAILED ; un obligatoire non exécuté →
NEEDS REVIEW ; sinon VALIDATED. READY FOR EXPORT : action de l'utilisateur, refusée hors VALIDATED.
Tolérances par défaut : RTP ±1e-6 ; hit ±1 % relatif ; fréquence des bonus ±1 % relatif ; part bonus
±0,5 point ; fréquence du max ±1 % relatif ; σ ±15 % relatif (avertissement) ; simulation ≥ 1 M spins ;
|z| ≤ 3,89 (p ≈ 1e-4) ; khi-deux p ≥ 1e-4 ; clôture ≤ 1e-9. Barèmes Stake : avertissements (non
bloquants chez Stake). Le max win n'est JAMAIS jugé sur sa présence en simulation : espérance N·p et
P(aucun) = (1−p)^N affichées, la probabilité exacte fait foi.

## D-10 — Simulation reproductible (2026-11-06)
xoshiro256++ amorcé par SplitMix64 ; lots de 2²⁰ spins ; lot i ↔ flux `stream(graine, i)` ; compteurs
entiers par issue (u32 par thread) additionnés : résultat identique quel que soit le nombre de threads
(empreinte `counts_hash`, contrôle 7.3). Tirage : table alias de Vose (O(1)). Plafond 4·10⁹ spins.
Modes de performance : Éco = ¼ des cœurs, Équilibré = ½, Performance = tous sauf un ; plafond réglable ;
jamais tous les cœurs.

## D-11 — Générateur de lois (2026-11-06)
Quatre classes (pas de gain, base, bonus, max). Probabilités et espérances de classe fixées par les
objectifs ; dans chaque classe, loi de puissance discrétisée x^−a sur une grille à pas croissants (0,1×
jusqu'à 10×, 0,5× → 100×, 5× → 1 000×, 50× → 10 000×, 500× au-delà), exposant résolu par dichotomie
pour l'espérance EXACTE. Le plancher du bonus règle la volatilité (dichotomie sur σ/prix visé). Si la
cible est hors d'atteinte, le générateur l'écrit et donne la plage atteignable. Fréquence du max par
défaut : 1,5 % du RTP au plafond (modifiable ; référence studio : ~1/1,4 M pour ×50 000). Poids entiers
sur 10¹⁵. C'est une **loi cible** (SPECIFICATION + EXACT) que Claude doit reproduire avec de vraies règles.

## D-12 — Moteur de règles (2026-11-06)
Rouleaux à bandes, ways (243…) ou lignes, wild (substitution, ways « que des wilds » non payées),
scatter (gains, tours gratuits, relances), multiplicateur des tours gratuits, plafond, achat de bonus.
Espérance EXACTE du jeu de base par formules fermées ; loi complète par énumération si Π longueurs ≤
5·10⁷ ; rounds complets par simulation. Mécaniques non modélisées par les règles en v1 (cascades,
clusters, Megaways, Hold & Win, gamble, progressifs) : traitées au niveau de la loi cible (forme
lookup table, indépendante de la mécanique) et décrites dans le prompt.

## D-13 — Documents (2026-11-06)
Dédoublonnage SHA-256 ; rangement par type ; texte : `pdftotext` (Git for Windows le fournit sur ce
poste) puis `pdf-extract` ; Excel par calamine. Pas d'OCR installé : images et PDF-images → saisie
manuelle. Toute valeur extraite ou saisie est « À VALIDER » et n'est utilisée qu'une fois validée.

## D-14 — Non-régression (2026-11-06)
Test `crates/smi-core/tests/non_regression.rs` : les 5 tables publiées de FINDERS KEEPERS doivent
redonner les chiffres de son analyse indépendante (Python, 2026-11-05) — RTP et hit au 1e-12, σ,
P(max), CVaR, ETL au 1e-9. Tout changement du moteur qui casse ce test est une régression.

## D-15 — Version macOS (2026-11-06)
Demande de l'utilisateur : « la MÊME application, mais pour Mac », dans un dossier à part
(`Desktop/SLOT-MATH-INTELLIGENCE-MAC`). Choix :
- **Même code, rendu portable** plutôt qu'une réécriture : moteurs, écrans et tests identiques ; seules
  les parties propres au système sont conditionnées (`paths::default_root`, emplacements de `pdftotext`,
  manifeste Windows décidé sur la CIBLE dans `build.rs`, polices système du Mac). La version Windows n'est
  pas modifiée.
- **Données dans `~/Documents/SLOT-MATH-INTELLIGENCE`** : visible dans le Finder (dépôt de documents),
  une seule autorisation d'accès demandée par macOS. Le Bureau n'est pas sondé (autorisation de plus).
- **Pack de départ embarqué** (`SOURCE/starter/` + `PROJECT/` + `DOCS/`, ressources Tauri) : installé au
  premier lancement en arrière-plan, copie SANS écrasement puis imports idempotents (module
  `smi-data::starter`, partagé avec `smi init`). Le modèle d'exemple est RÉGÉNÉRÉ par le moteur depuis sa
  spécification (pas de copie de base : les chemins enregistrés sont absolus et propres à chaque machine).
- **Paquet** : application universelle (Apple Silicon + Intel), `.dmg`, signature ad hoc (`signingIdentity
  "-"`, obligatoire pour lancer un binaire Apple Silicon téléchargé) ; pas de notarisation (il faudrait un
  compte développeur Apple payant) → première ouverture via « Ouvrir quand même » (INSTALLATION_MAC.md).
- **Fabrication** : impossible depuis Windows (SDK et éditeur de liens d'Apple). Deux voies : action GitHub
  `.github/workflows/macos.yml` sur une machine macOS (tests sur macOS, paquet vérifié, premier lancement
  simulé avec le pack embarqué) ou `construire-mac.sh` sur un Mac.
