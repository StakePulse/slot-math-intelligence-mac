# Guide d'utilisation

## Le flux de travail

1. **Market Intelligence** — charger et lire la référence du marché.
2. **Math Generator** — choisir le max win (×500, ×1 000, ×5 000, ×10 000, ×50 000…), le type, les
   mécaniques, le RTP, le hit rate, les bonus, la volatilité → loi exacte en direct, comparée aux pairs.
3. **Math Engine** — analyse exacte complète, puis **Valider** (simulation, reproductibilité, 9 étapes).
4. **Spin Simulator** — simulations à graine, sessions, bankroll, comparaison de configurations.
5. **Prompt Generator** — copier ou exporter le cahier des charges pour Claude.
6. **Math Library** — versions, historique, comparaison de deux versions.

## Lire un chiffre : sa nature

Chaque carte porte une étiquette :
- **EXACT** : somme pondérée finie sur une loi (lookup table publiée ou générée). Pas d'incertitude.
- **EXACT FORMULA** : formule fermée sur valeurs exactes (attente médiane, « au moins une fois en n »).
- **MONTE CARLO** : tirages au hasard ; graine, nombre de spins et intervalles de confiance affichés.
- **IMPORTÉ** : chiffre d'une source tierce (ex. instantané slotlab du 2026-11-03), non recalculable ici.
- **SPECIFICATION / CONFIGURATION** : objectifs et réglages de conception, jamais des mesures.

## Écrans

**Dashboard** — compteurs, dernière collecte, alertes de cohérence (à marquer « traitées »), projets récents, journal.
Au tout premier lancement, un bandeau suit l'installation des données de départ dans le dossier `Documents/SLOT-MATH-INTELLIGENCE`.

**Market Intelligence** — filtres (nature du mode, volatilité, plage de max win, plage de RTP, top N, provider).
- *Référence statistique* : pour chaque indicateur, n, moyenne, médiane, moyennes pondérées (bets / turnover),
  écart-type, P10, Q1, Q3, P90, min–max, nombre de valeurs extrêmes ; « n insuffisant » sous 10 jeux.
  Cliquer un indicateur affiche sa distribution.
- *Classement* : rang aux **bets sur 30 jours** (n.d. si la source ne les donne pas) et, à part, rang et part du turnover ; évolution du rang entre deux collectes.
- *Providers* : comparaison multi-studios (σ, max, hit, RTP médians).
- *Sources et collectes* : chaque donnée est datée et rattachée à sa collecte.
- **Collecte automatique StakeCruncher : désactivée par défaut.** Le `robots.txt` du site interdit son API aux
  robots ; l'activer (Settings → Collecte) est votre décision. En attendant : « Importer un instantané »
  (fichier `market.json` d'un tiers) et « Importer une lookup table » (`id,weight,payout`).

**Slot Explorer** — une slot et ses modes. Avec une lookup table : analyse exacte complète (classes,
bandes, percentiles, seuils, barèmes Stake) et bouton « Simuler ce mode ». Sans table : chiffres importés.

**Math Generator** — la loi est recalculée à chaque modification. Le panneau *Références du marché* donne
les médianes des jeux comparables (même nature, plafond à ×2 près) et peut les **appliquer** (choix de
conception assumé, tracé dans le prompt). *Construction* montre les paramètres résolus et la plage de
volatilité atteignable : si la volatilité demandée est hors d'atteinte, une note l'explique. Réglages
avancés : σ visé explicite, part du RTP au max, gain max du jeu de base, plancher du bonus.
« Enregistrer » crée un modèle (ou une nouvelle version du modèle ouvert).

**Math Engine**
- *Analyse exacte* : indicateurs, classes, catégories (petits / moyens / gros / très gros), clôture du RTP
  par 5 sommations, bandes (probabilité log + contribution au RTP), lois du jeu de base et des bonus,
  seuils et attentes, barèmes Stake 2★/3★.
- *Validation* : lance (ou réutilise) une simulation ≥ 1 M spins, vérifie la reproductibilité (1 puis 3
  threads, mêmes compteurs), puis les 9 étapes. Statuts : DRAFT, NEEDS REVIEW (contrôle obligatoire non
  exécuté), VALIDATION FAILED, VALIDATED, READY FOR EXPORT (action manuelle, seulement après VALIDATED).
- *Moteur de règles* : décrire un jeu à rouleaux en JSON (ways ou lignes, wild, scatter, tours gratuits,
  multiplicateur, plafond), obtenir l'espérance EXACTE du jeu de base (et la loi complète par énumération si
  elle est faisable), puis simuler des rounds complets.

**Spin Simulator** — cible (modèle, mode du marché avec table, jeu à rouleaux), nombre de spins (100 k à
100 M ou libre), graine, threads. Pause / reprise / annulation. Résultats : exact vs simulé (z, IC, Wilson,
khi-deux), spins par catégorie, convergence du RTP, seuils observés / attendus avec P(aucun) pour les
événements rares, loi empirique complète. *Sessions et bankroll* : retour d'une session de N rounds,
percentiles avec IC, survie d'une bankroll. *Comparer* : plusieurs simulations côte à côte, la plus proche
des objectifs de son modèle signalée.

**Prompt Generator** — aperçu rendu ou brut, Copier, export .md / .txt, données JSON + lookup table de la
loi cible, rapport de validation. Si la loi n'est pas VALIDATED, le prompt le dit.

**Document Library** — import (PDF, images, CSV, JSON, TXT, Excel), dépôt dans `DOCUMENTS/INBOX` (Explorateur ou Finder) puis
« Intégrer », recherche plein texte, notes et étiquettes. Les valeurs repérées sont « À VALIDER » :
Valider, Corriger ou Rejeter. Images et PDF sans texte : saisie manuelle (pas d'OCR installé). Un CSV
`id,weight,payout` est reconnu comme lookup table et peut être analysé.

**Math Library** — modèles (UUID, dates, versions, sources, statut), historique des versions,
simulations du modèle, comparaison champ par champ de deux versions.

**Settings** — modes Économique (¼ des cœurs) / Équilibré (½) / Performance (tous sauf un), plafond de
threads, graine par défaut ; collecte ; référence du marché ; seuils (volatilité, tolérances, catégories,
barèmes) ; chemins et vérification de la structure.

## Ce que l'outil ne fait pas (v0.2)
- Pas de collecte automatique sans votre accord (robots.txt).
- Pas de statistiques de bonus ni de scatters pour les jeux du marché : une lookup table publique ne dit
  pas quel round est un bonus (affiché « non disponible »).
- Pas d'OCR : saisie manuelle des valeurs d'images.
- Le moteur de règles ne simule pas encore cascades, clusters, Megaways, Hold & Win, gamble, progressifs ;
  ces mécaniques sont traitées au niveau de la loi cible et décrites dans le prompt.

## En ligne de commande
`smi` (`smi.exe` sous Windows ; sur Mac, fourni à côté du `.dmg`) : `init [pack]`, `stats <table.csv> [prix]`, `generate <spec.json>`,
`revise <modèle> <spec.json> [note]`, `simulate|validate <modèle> [spins] [graine]`, `prompt <modèle>`,
`docs`, `collect`, `market`.
