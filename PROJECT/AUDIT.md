# Phase 1 — Audit (2026-11-06)

## 1. Documents analysés

| Document | Rangé dans | Nature | Ce qu'on en retient |
|---|---|---|---|
| Witchcraft_Mathematical_Model_v2-1.pdf (85 p.) | `DOCUMENTS/PDF` | Modèle mathématique publié d'un slot 256 ways, 8 modes, ×50 000 | Structure de référence (voir §2) |
| TempleDrop_Mathematical_Model_v1.pdf (18 p.) | `DOCUMENTS/PDF` | Même format, cluster 6×6 à tumbles, 7 modes, ×50 000 | Même méthode, version courte : la structure se décline en 18 pages |
| FINDERS_KEEPERS_redistribution_2026-11-05.pdf (9 p., images) | `DOCUMENTS/PDF` | Notre rapport : loi exacte des tables + tirages simulés + sessions + barèmes Stake | Pas de couche texte (rendu PIL) : il faut l'OCR ou la saisie manuelle ; ses données sources existent en JSON (`finders-keepers/reports/redistribution/donnees.json`) |
| Witchcraft_Mathematical_Model_v2.pdf | — | Identique octet pour octet à v2-1 (vérifié par `cmp`) | Non dupliqué |
| 6 captures du 2026-11-06 | `DOCUMENTS/SCREENSHOTS` | Conseil de McSon (« les 30-50 slots les plus jouées… teste à fond ta slot »), modules demandés, exemple ×5 000, arborescence conseillée, tableau des modes de FK, The Dog House | Cahier des charges informel |
| slotlab-main.zip (outil tiers, 2026-11-04) | `DOCUMENTS/OTHER` | Outil d'analyse Stake Engine d'un tiers (Rust + Svelte) | Montre COMMENT StakeCruncher sert ses données (§3) et contient un instantané de marché |
| `market.json` de slotlab (2026-11-03) | `DATABASE/imports` | 125 slots, 1 116 modes, chiffres calculés sur les tables publiées | Importable comme **source tierce datée**, jamais présentée comme notre collecte |

## 2. Méthode extraite des modèles Witchcraft / Temple Drop

1. **Deux couches** : les *books* disent ce qui PEUT arriver (rounds complets, conformes aux règles) ; la *lookup table* (un poids entier par book) dit À QUELLE FRÉQUENCE. `P(book i) = w_i / W`, `P(E) = Σ_{i∈E} w_i / W`. Toute statistique par round est une somme pondérée finie, donc **exacte**.
2. **Étiquette de nature sur chaque chiffre** : EXACT (fichiers publiés), EXACT FORMULA (formule fermée sur valeurs exactes, ex. `1-(1-p)^n`), MONTE CARLO (sessions, survie de bankroll, avec IC à 95 %), CONFIGURATION (bandes, tables des carrés spéciaux : entrées de simulation, elles ne fixent PAS les fréquences), SPECIFICATION (règles et cibles). → repris tel quel dans l'application (`Nature` dans `smi-core`).
3. **Indicateurs par mode** : prix, RTP, hit rate, P(gain ≥ prix), médiane, SD/prix, CV, VI90 = 1,645·σ/c, VI95, skewness, kurtosis en excès, percentiles P25…P99,99, P(≥ k × prix) pour k = 0,5…50, concentration (part du RTP portée par le top x % des rounds), probabilité et RTP du max win, livres au plafond, « effective books » = (Σw)²/Σw², livre le plus lourd.
4. **Classes de résultat** (no win, gains de base, witch, bonus 3/4/5 scatters) : partition, fréquence, points de RTP, moyenne/médiane/P10-P90/P(no win) **conditionnels**, max win conditionnel. Les points de RTP des classes se somment au RTP.
5. **Bandes de gain** (0 ; 0,1–0,4 ; 0,5–0,9 ; 1–1,9 ; … ; 20 000–49 999,9 ; max) : probabilité, 1 sur N, pour 10 000 rounds, points de RTP, moyenne dans la bande, survie au début de bande, nombre de books.
6. **Seuils et attentes** : `P(≥x)`, 1 sur N, attente médiane `ln 0,5 / ln(1-p)`, « au moins une fois en n rounds » `1-(1-p)^n`, dépense moyenne par événement = prix / p.
7. **Max win** : probabilité exacte par mode, origine par classe, RTP au plafond = plafond × p / prix, troncature mesurée (borne inférieure pour les bonus).
8. **Monte Carlo seulement là où il n'y a pas de forme fermée** (retour d'une session de N rounds, survie de bankroll) : tirage alias à graines fixes, IC de Wilson et IC d'ordre pour les percentiles ; validation du MC contre l'exact (`z = (sim-exact)/sqrt(exact(1-exact)/n)`, ~95 % des |z| ≤ 1,96 attendus).
9. **Vérification** : intégrité des fichiers (ids, payouts, SHA-256), ré-évaluation de chaque book par un programme indépendant, vérificateur lui-même validé sur 18 corruptions volontaires, **clôture des décompositions** (le RTP obtenu par 4 sommations différentes doit coïncider), recoupement avec le fichier de statistiques du moteur (piège relevé : skewness/kurtosis du moteur standardisés avec σ par unité de prix → faux d'un facteur c³ / c⁴ hors mode à prix 1).

## 3. StakeCruncher : ce qui est accessible, et comment

Constaté le 2026-11-06 (requêtes `curl` uniquement, aucun navigateur) :

- **Toutes les pages publiques** (`/slots-tracker`, `/slots-tracker/g/<slug>`, `/slots-tracker/stats/<slug>`, `/terms`) renvoient la même coquille HTML de 24 852 octets : **aucune donnée sans JavaScript**. `/terms` est aussi rendue en JS : conditions d'utilisation illisibles sans navigateur.
- Les données viennent d'une API JSON : `/tracker-api/engine/games?sort=…&limit=…&offset=…` (classement : `turnover_*`, `bet_count_*` sur hier / semaine / 30 j / 90 j / année / total, joueurs actuels et pics, RTP affiché, date de mise à jour), `/tracker-api/engine/verifier/catalog` (modes et versions de chaque jeu), `/tracker-api/engine/verifier/lookup-table?slug&version&mode` (la table publiée : `id,weight,payout`, coût du mode en en-tête `x-lookup-cost-multiplier`). Limite annoncée : `X-Ratelimit-Limit: 600`, cache 45 s.
- **`robots.txt` : `Disallow: /tracker-api/`**, commenté « private JSON API paths ». Les robots sont bienvenus partout ailleurs, mais les pages autorisées ne contiennent pas les données.
- `sitemap-slots.xml` : 7 064 fiches de jeu ; `sitemap-cruncher.xml` : 7 116 fiches de stats ; `sitemap-providers.xml` : 721 studios.

**Conséquence (décision D-03)** : la collecte automatique sur `/tracker-api/` est codée mais **désactivée par défaut** ; l'activer est une décision de l'utilisateur, affichée avec l'avertissement `robots.txt`. Sans elle, l'application fonctionne avec : l'instantané tiers daté (slotlab, 2026-11-03), l'import manuel de tables `id,weight,payout` (fichiers `lookUpTable_*.csv` de nos propres jeux ou téléchargés à la main), et l'import CSV/JSON. Rien n'est inventé : toute donnée absente est affichée « non disponible ».

### Ce que chaque source peut fournir

| Donnée demandée | API StakeCruncher (opt-in) | Table `id,weight,payout` | Instantané slotlab |
|---|---|---|---|
| Nom, provider, rang | oui | non (nom saisi) | oui (rang au turnover) |
| Nombre de bets 7 / 30 / 90 j | oui (`bet_count_past_week/30d/rolling_90d`) | non | **non** |
| Turnover 7 / 30 / 90 j | oui | non | part du turnover seulement |
| Joueurs (actuels, pics) | oui | non | non |
| Historique / évolution du rang | à construire par nos collectes datées | non | non |
| RTP, hit, bust, max win et sa probabilité, percentiles, paliers, SD | calculés par NOTRE moteur sur la table | **exact** | chiffres tiers (non recalculables) |
| Fréquence des bonus, répartition base/bonus | **non** : une table ne dit pas quel round est un bonus | non | non |
| Scatters, anticipation, durée des features | **non** (il faut les books, non servis) | non | non |

Les statistiques de bonus du marché ne sont donc **pas disponibles** en v1 (affiché comme tel). Elles le deviennent pour nos propres jeux, dont nous avons les books.

## 4. Environnement

Windows 11, 16 cœurs logiques, Rust 1.95 (MSVC), Node 24, npm 11, Visual Studio 2022, WebView2. Pas de pnpm, pas de Tesseract (pas d'OCR local : les images sont classées et les valeurs se saisissent/valident à la main). Une application Tauri 2.12 compile déjà sur ce poste (Patisserie) : versions reprises.

## 5. Risques identifiés

| Risque | Parade |
|---|---|
| Collecte contraire au souhait du site (`robots.txt`) | Opt-in explicite, débit poli (≥ 1 s entre requêtes, arrêt sur 429), cache, User-Agent identifiable |
| Changement de structure de l'API | Validation de schéma à chaque collecte, champs manquants → « non disponible », alerte de structure enregistrée |
| Confondre turnover et popularité | Classement par **bets 30 j** ; turnover affiché à côté, jamais fusionné |
| Moyenne aveugle | Toujours moyenne + médiane + pondérées (bets, turnover) + quartiles + n ; avertissement n < 10 |
| Événements rares (max win) jugés sur une simulation | Probabilité exacte d'abord ; en simulation : espérance N·p et P(0 occurrence) = (1-p)^N affichées, jamais « erreur » |
| Volatilité « à l'impression » | Score composite reproductible, seuils dans les réglages |
| Saturation du PC | Modes Éco / Équilibré / Performance, plafond de threads, lots, annulation |
| Valeurs extraites des PDF fausses | Statut « à valider » obligatoire, jamais utilisées avant validation humaine |
