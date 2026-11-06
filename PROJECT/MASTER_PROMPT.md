# PROJET : SLOT MATH INTELLIGENCE — MASTER DEVELOPMENT PROMPT — VERSION 1.0

> Référence permanente du projet. Texte fourni par l'utilisateur le 2026-11-06, recopié tel quel
> (seule la mise en forme Markdown a été ajoutée). Les décisions prises pour l'appliquer sont dans
> `PROJECT/DECISIONS.md`, l'audit de départ dans `PROJECT/AUDIT.md`, l'avancement dans `PROJECT/ROADMAP.md`.

Tu es un ingénieur logiciel senior spécialisé dans les applications desktop, Rust, les moteurs mathématiques, les simulations statistiques, l'analyse de données et les logiciels de création de jeux de casino.

Ta mission est de concevoir et développer une application Windows professionnelle appelée Slot Math Intelligence.

Ce projet n'est pas un simple générateur de prompts. Il doit devenir un véritable laboratoire mathématique permettant d'étudier le marché des slots, de comprendre leurs distributions, de concevoir de nouveaux modèles mathématiques, de simuler des millions de spins et de générer des spécifications mathématiques complètes destinées à Claude pour nos futurs projets de slots.

Tu dois développer une véritable application fonctionnelle, évolutive et performante.

## 1. OBJECTIF PRINCIPAL

Construire un logiciel capable de :

1. Explorer et analyser les données de StakeCruncher.
2. Identifier les 30 à 50 slots les plus populaires sur les 30 derniers jours.
3. Étudier leurs caractéristiques mathématiques.
4. Calculer des références statistiques par provider et par catégorie de volatilité.
5. Identifier les tendances du marché.
6. Créer des modèles mathématiques pour de nouvelles slots.
7. Simuler des millions de spins localement sur le PC.
8. Comparer les résultats théoriques et expérimentaux.
9. Détecter automatiquement les incohérences mathématiques.
10. Générer des prompts extrêmement détaillés pour Claude.
11. Enregistrer toutes les données, analyses et simulations pour constituer une bibliothèque évolutive.

Le logiciel doit fonctionner localement sur Windows.
Les calculs doivent être réalisés localement, sans dépendre d'un serveur distant pour les simulations.

## 2. SOURCES DE DONNÉES

Source principale : StakeCruncher — Site : https://stakecruncher.com/

Explorer en priorité les sections suivantes :

- https://stakecruncher.com/slots-tracker
- https://stakecruncher.com/slots-tracker/stats

L'objectif est d'exploiter au maximum les informations accessibles.

### A. Stake Engine Tracker

Récupérer autant que possible :

- Nom de la slot.
- Provider.
- Classement par popularité.
- Turnover.
- Nombre de bets.
- Nombre de joueurs.
- Statistiques sur 7 jours.
- Statistiques sur 30 jours.
- Statistiques sur 90 jours.
- Statistiques historiques.
- Historique de performance.
- Évolution du classement.

### B. Slot Cruncher

Récupérer les données mathématiques disponibles :

- RTP théorique.
- RTP par mode.
- Volatilité.
- Hit rate.
- Bust rate.
- Max win.
- Informations sur les bonus.
- Distributions des gains.
- Informations disponibles sur les probabilités.
- Données des lookup tables lorsqu'elles sont publiquement accessibles.
- Statistiques détaillées des différents modes.

### C. Méthode de collecte

Le logiciel doit :

- Automatiser la collecte des données accessibles.
- Identifier les changements de structure du site.
- Gérer les erreurs et les données indisponibles.
- Conserver les données historiques.
- Éviter les doublons.
- Identifier la date de chaque collecte.
- Permettre une actualisation manuelle et automatique.
- Respecter les limitations d'accès et les conditions d'utilisation du site.

Ne jamais inventer de données manquantes.
Si une donnée n'est pas accessible, l'indiquer explicitement.
Ne pas contourner de protections, de CAPTCHA ou de restrictions d'accès.

### 30 à 50 slots de référence

Le logiciel doit pouvoir identifier les 30 à 50 slots les plus pertinentes sur une période glissante de 30 jours.
Utiliser prioritairement le nombre de bets pour mesurer la popularité, et conserver le turnover comme indicateur complémentaire.

Il doit être possible de filtrer :

- Tous les providers.
- Un provider particulier.
- Une catégorie de volatilité.
- Une plage de Max Win.
- Une plage de RTP.

Ne pas confondre les slots les plus jouées avec celles ayant le turnover le plus élevé.

### Attention à la représentativité

Ne jamais faire une moyenne aveugle de toutes les slots.
Calculer séparément :

- Moyenne arithmétique.
- Médiane.
- Moyenne pondérée par le nombre de bets.
- Moyenne pondérée par le turnover.
- Quartiles.
- Percentiles.
- Écart-type.
- Distribution des valeurs.

Identifier les valeurs extrêmes et les échantillons insuffisants.
Les statistiques commerciales observées sur 30 jours ne doivent jamais être confondues avec le RTP théorique ou les probabilités mathématiques d'un jeu.

## 3. ANALYSE MATHÉMATIQUE DU MARCHÉ

Créer un module complet appelé MARKET INTELLIGENCE.
Il doit être capable de construire une véritable référence du marché.

### Statistiques à analyser

Pour chaque slot :

- RTP.
- Hit frequency.
- Fréquence des gains supérieurs ou égaux à 1x.
- Fréquence des gains supérieurs ou égaux à 2x.
- Fréquence des gains x5, x10, x20, x50, x100, x500, x1 000 et plus.
- Max Win.
- Fréquence des bonus.
- Distribution des bonus.
- Répartition du RTP entre Base Game et Bonus.
- Volatilité.
- Écart-type.
- Moyenne des gains.
- Médiane des gains.
- Percentiles P10, P25, P50, P75, P90, P95 et P99 lorsque disponibles.
- Fréquence des gros gains.
- Fréquence des spins perdants.
- Informations sur les scatters.
- Scatter anticipation, lorsque disponible.
- Durée et fréquence des fonctionnalités, lorsque documentées.

Ne pas prétendre disposer de ces statistiques lorsqu'elles ne sont pas publiées ou calculables à partir des données disponibles.

### Classification de volatilité

Créer une classification :

- Low.
- Medium-Low.
- Medium.
- Medium-High.
- High.
- Very High.

Ne pas attribuer une volatilité uniquement à partir d'une impression ou d'une étiquette commerciale.
Construire une classification reproductible reposant sur plusieurs indicateurs mathématiques :

- Écart-type.
- Fréquence des gains.
- Part du RTP provenant des gros gains.
- Distribution des gains.
- Fréquence des pertes.
- Probabilités des différents paliers.

Les seuils de classification devront être configurables et documentés.

### Comparaison entre providers

Afficher :

- Les providers les plus populaires.
- Leurs slots les plus jouées.
- Leur Max Win moyen et médian.
- Leur RTP moyen.
- Leur volatilité.
- Leur fréquence de hit.
- Leur distribution des gains.

Permettre de comparer plusieurs providers simultanément.

## 4. MATH GENERATOR

Créer un module central appelé MATH GENERATOR.
Il doit permettre à l'utilisateur de définir les caractéristiques de sa future slot.

### Paramètres obligatoires

- Nom du projet.
- Multiplicateur maximum souhaité.
- RTP cible.
- Volatilité souhaitée.
- Type de slot.
- Mécaniques de jeu.
- Fréquence de hit souhaitée.
- Fréquence des bonus.
- Répartition du RTP entre Base Game et Bonus.

### Multiplicateur maximum

Proposer des valeurs rapides : x100, x250, x500, x1 000, x2 000, x5 000, x10 000, x20 000, x50 000, Personnalisé.
L'utilisateur doit pouvoir saisir n'importe quel multiplicateur.

### Mécaniques configurables

Prévoir notamment : Classic Reels, Ways, Paylines, Megaways, Cascading Reels, Cluster Pays, Expanding Wilds, Sticky Wilds, Multipliers, Free Spins, Bonus Games, Scatter Features, Buy Bonus, Gamble Features, Hold and Win, Progressive Features.

Permettre de combiner plusieurs mécaniques.

### Références du marché

Lorsqu'un utilisateur demande une slot x5 000 :

1. Identifier les slots du marché présentant des caractéristiques comparables.
2. Identifier leurs distributions mathématiques.
3. Comparer leurs fréquences de gains.
4. Comparer leurs volatilités.
5. Calculer les moyennes et médianes pertinentes.
6. Identifier les différences entre les providers.
7. Construire une proposition mathématique adaptée à notre objectif.

Ne pas simplement copier une moyenne de marché.
Utiliser les références comme points de comparaison et produire une proposition cohérente avec les contraintes du nouveau jeu.

### Génération de distributions

Le moteur doit pouvoir proposer :

- Distribution Base Game.
- Distribution Bonus.
- Distribution des petits gains.
- Distribution des gains moyens.
- Distribution des gros gains.
- Distribution des très gros gains.
- Probabilité du Max Win.
- Répartition du RTP.

Chaque distribution doit être accompagnée de ses probabilités et de sa contribution au RTP.

## 5. MATH ENGINE

C'est le cœur mathématique de l'application. Il doit être développé en Rust.
Le moteur doit être capable de construire et de vérifier des modèles mathématiques complets.

### Calculs indispensables

- RTP : `RTP = Σ(P(i) × Gain(i))`
- Hit Frequency : `HF = P(Gain > 0)`
- Probabilité d'un événement : `P(E) = Σ P(i)`, pour tous les résultats appartenant à E.
- Fréquence théorique : `Frequency = 1 / P(E)`
- Contribution au RTP : `RTP contribution = P(i) × Gain(i)`

Développer également : variance, écart-type, moments statistiques, skewness, kurtosis, distribution cumulée, percentiles, probabilité des gains supérieurs à différents seuils, probabilité du Max Win, fréquence des bonus, espérance mathématique des bonus.

Toutes les formules doivent être documentées et testées.

### Cohérence mathématique

Le moteur doit notamment vérifier :

- Que la somme des probabilités vaut 100 %.
- Que les contributions RTP correspondent au RTP annoncé.
- Que la distribution respecte le Max Win.
- Que les bonus respectent leurs probabilités de déclenchement.
- Que les différents modes de jeu sont cohérents.
- Que les fréquences annoncées correspondent aux probabilités.
- Que les distributions conditionnelles sont correctement calculées.
- Que les probabilités ne sont pas négatives.
- Que les gains ne dépassent jamais le plafond prévu.

Aucun modèle ne doit être considéré comme validé si ces contrôles échouent.
Utiliser une précision numérique adaptée, et privilégier les calculs déterministes lorsque cela est possible.

## 6. SPIN GENERATOR & TESTING ENGINE

Créer un module permettant de générer des spins localement sur le PC. C'est une fonctionnalité essentielle.

### Nombre de spins

100 000 ; 500 000 ; 1 million ; 5 millions ; 10 millions ; 50 millions ; 100 millions ; nombre personnalisé.
Le moteur doit pouvoir traiter des volumes importants sans saturer inutilement la mémoire.

### Types de simulations

Base Game ; Free Spins ; Bonus ; Bonus Buy ; Modes spéciaux ; différentes configurations de volatilité ; différentes configurations mathématiques.

Les simulations doivent reproduire les règles du modèle mathématique, et non générer artificiellement des gains pour obtenir le RTP souhaité.

### Résultats à afficher

Nombre de spins ; RTP obtenu ; RTP théorique ; Hit Frequency ; nombre de spins perdants ; nombre de petits gains ; nombre de gains moyens ; nombre de gros gains ; nombre de gains supérieurs à x10, x50, x100, x500, x1 000 ; plus gros gain obtenu ; fréquence des bonus ; moyenne des gains bonus ; médiane des gains bonus ; distribution des gains ; écart entre théorie et simulation.

### Comparaison des simulations

Permettre de comparer plusieurs configurations. Exemple :

- Configuration A : RTP 96,5 % ; volatilité Medium ; Max Win x5 000 ; 10 millions de spins.
- Configuration B : RTP 96,5 % ; volatilité High ; Max Win x5 000 ; 10 millions de spins.

Comparer tous les indicateurs et identifier la configuration la plus proche des objectifs.

### Tests statistiques

Intervalles de confiance ; tests de convergence ; comparaison théorique / empirique ; tests de fréquence ; détection des anomalies ; vérification de la reproductibilité.
Utiliser des seeds configurables pour pouvoir reproduire les mêmes simulations.

### Attention aux événements rares

Un événement extrêmement rare comme un Max Win x50 000 ne peut pas être validé uniquement par une simulation de 10 millions de spins.
Le moteur doit donc disposer de méthodes complémentaires : calcul exact des probabilités lorsque le modèle le permet ; énumération des résultats possibles lorsque c'est réalisable ; analyse des distributions pondérées ; tests ciblés des événements rares ; vérification des plafonds et des règles de paiement.

Ne jamais présenter une absence de Max Win en simulation comme une preuve d'impossibilité ou une erreur mathématique.

## 7. ARCHITECTURE TECHNIQUE OBLIGATOIRE

### Rust

Rust doit être utilisé obligatoirement pour : Math Engine ; Spin Generator ; Simulation Engine ; calculs statistiques ; traitement des distributions ; validation mathématique ; traitement des gros volumes de données.
Ne pas remplacer Rust par Python pour les calculs principaux.

### Application Windows

Architecture recommandée : Tauri 2 pour l'application desktop ; Rust pour le backend et les moteurs ; React + TypeScript pour l'interface ; SQLite pour la base de données locale.
Cette architecture pourra évoluer si une meilleure solution technique est justifiée.

### Optimisation des ressources

Multithreading configurable ; nombre de threads ajustable ; traitement par lots ; gestion intelligente de la mémoire ; écriture progressive des résultats ; annulation des tâches ; pause et reprise lorsque techniquement possible ; barre de progression ; gestion des erreurs ; traitement asynchrone pour ne jamais bloquer l'interface.

### Modes de performance

- ÉCONOMIQUE : faible utilisation CPU ; nombre de threads réduit ; priorité à la fluidité du PC.
- ÉQUILIBRÉ : bon compromis entre rapidité et consommation.
- PERFORMANCE : utilisation des ressources disponibles dans les limites configurées.

L'utilisateur doit pouvoir personnaliser le nombre maximal de threads.
Ne jamais monopoliser inutilement tous les cœurs du processeur.

## 8. PROMPT GENERATOR

Créer un générateur de prompts destiné à Claude. C'est le résultat final de notre workflow.
Après avoir défini une slot et validé ses objectifs mathématiques, l'application doit pouvoir générer un prompt extrêmement détaillé.

Ce prompt doit contenir :

- **Informations générales** : nom de la slot ; type de jeu ; mécaniques ; RTP cible ; Max Win ; volatilité ; fréquence de hit souhaitée ; fréquence des bonus.
- **Informations mathématiques** : distribution Base Game ; distribution Bonus ; distribution des gains ; contribution RTP de chaque catégorie ; probabilités ; fréquences ; percentiles ; écart-type ; probabilité du Max Win ; répartition du RTP ; résultats des simulations.
- **Références du marché** : slots comparables ; providers comparables ; moyennes ; médianes ; distributions de référence ; caractéristiques de volatilité ; justification des choix mathématiques.
- **Contraintes techniques** : architecture mathématique ; règles de paiement ; gestion des bonus ; gestion des multiplicateurs ; plafond de gain ; méthode de vérification ; objectifs de validation.

Le prompt doit demander à Claude de produire un modèle mathématique cohérent et vérifiable, et non simplement de proposer des nombres arbitraires.

### Export

Copier le prompt ; exporter en TXT ; exporter en Markdown ; exporter les données mathématiques en JSON ; exporter les rapports de validation.

## 9. IMPORTATION DE DOCUMENTS

Créer un module DOCUMENT LIBRARY.
L'utilisateur doit pouvoir importer : PDF ; images ; screenshots ; CSV ; JSON ; TXT ; Excel.
L'application doit pouvoir classer les documents et conserver leurs références.

Les PDF mathématiques doivent être analysés afin d'extraire : RTP ; probabilités ; fréquences ; distributions ; tableaux ; formules ; statistiques ; paramètres des modes ; informations sur les bonus.

Prévoir une validation humaine des informations extraites.
Ne jamais considérer automatiquement une valeur OCR comme fiable sans contrôle.
Créer une bibliothèque consultable et réutilisable par les futurs projets.

## 10. BIBLIOTHÈQUE MATHÉMATIQUE ÉVOLUTIVE

Créer un système central permettant de sauvegarder : les modèles mathématiques ; les distributions ; les références du marché ; les slots analysées ; les providers ; les simulations ; les prompts générés ; les rapports de validation ; les versions des modèles.

Chaque modèle doit posséder : un identifiant unique ; une date de création ; un historique des modifications ; une version ; un statut de validation ; ses sources ; ses paramètres.

Permettre de comparer deux versions d'une même slot.
L'objectif est de construire progressivement une véritable base de connaissances mathématiques.

## 11. INTERFACE GRAPHIQUE

Créer une interface moderne, sombre et professionnelle. Je veux un logiciel qui ressemble à un véritable outil d'analyse technique.

### Menu principal

1. Dashboard. 2. Market Intelligence. 3. Slot Explorer. 4. Math Generator. 5. Math Engine. 6. Spin Simulator. 7. Prompt Generator. 8. Document Library. 9. Math Library. 10. Settings.

### Dashboard

Nombre de slots analysées ; nombre de providers ; nombre de modèles mathématiques ; nombre de simulations réalisées ; dernière actualisation des données ; dernières analyses ; projets récents ; alertes de cohérence mathématique.

### Graphiques

Distribution des gains ; courbes de RTP ; comparaisons de volatilité ; histogrammes ; courbes de convergence ; distribution des bonus ; comparaisons entre providers.
Les graphiques doivent être interactifs et lisibles.

## 12. CONTRÔLE QUALITÉ ET VALIDATION

Créer un système de validation indépendant. Chaque modèle doit passer par plusieurs étapes :

1. Vérification des probabilités. 2. Vérification du RTP. 3. Vérification du Max Win. 4. Vérification des bonus. 5. Vérification des distributions. 6. Vérification des fréquences. 7. Simulation. 8. Comparaison théorie / simulation. 9. Rapport final.

Statuts possibles : DRAFT ; NEEDS REVIEW ; VALIDATION FAILED ; VALIDATED ; READY FOR EXPORT.

Ne jamais afficher VALIDATED si les contrôles obligatoires n'ont pas été réalisés.
Les tolérances doivent être configurables et documentées.
Créer des tests unitaires et des tests d'intégration pour les fonctions mathématiques.
Prévoir des tests de non-régression afin de ne pas casser les modèles précédemment validés.

## 13. DOCUMENTS FOURNIS

Des documents PDF et des captures d'écran seront fournis dans le projet. Les premiers documents comprennent notamment :

- FINDERS_KEEPERS_redistribution_2026-11-05.pdf
- Witchcraft_Mathematical_Model_v2-1.pdf
- Des captures d'écran et échanges techniques concernant l'analyse des slots.

Tu dois analyser ces documents en profondeur. Ne pas te limiter à leurs premières pages.
Extraire leurs méthodes, formules, distributions et indicateurs.
Le document Witchcraft doit notamment servir d'exemple de structure de documentation mathématique approfondie.

Étudier en particulier : les modes de jeu ; les distributions pondérées ; les books ; les lookup tables ; les probabilités exactes ; les statistiques conditionnelles ; les calculs de Max Win ; les contrôles de cohérence ; les simulations Monte Carlo.

Distinguer systématiquement : les données exactes ; les estimations Monte Carlo ; les configurations ; les spécifications ; les objectifs de conception.

Ne pas recopier aveuglément les mathématiques de ces slots : comprendre les méthodes et construire un moteur générique.

## 14. MÉTHODE DE DÉVELOPPEMENT

Tu dois travailler de manière autonome et méthodique. Ne te contente pas de produire un cahier des charges ou une maquette. Tu dois réellement développer l'application.

- PHASE 1 — AUDIT : examiner les fichiers disponibles ; explorer StakeCruncher ; identifier les données accessibles ; étudier les contraintes techniques ; définir l'architecture ; identifier les risques.
- PHASE 2 — FONDATIONS : créer le projet ; installer les dépendances ; mettre en place Rust ; mettre en place Tauri ; créer la base SQLite ; créer l'interface ; vérifier que l'application démarre.
- PHASE 3 — MARKET INTELLIGENCE : développer la collecte, le stockage, les classements et les analyses.
- PHASE 4 — MATH ENGINE : développer les calculs mathématiques et les contrôles de cohérence.
- PHASE 5 — SPIN SIMULATOR : développer la génération de spins, le multithreading et les analyses statistiques.
- PHASE 6 — MATH GENERATOR : développer la création des modèles mathématiques.
- PHASE 7 — PROMPT GENERATOR : développer l'export complet des prompts destinés à Claude.
- PHASE 8 — DOCUMENT LIBRARY : développer l'importation et l'analyse des documents.
- PHASE 9 — TESTS ET OPTIMISATION : tester tous les modules ; vérifier les calculs ; tester les performances ; contrôler la consommation mémoire et CPU ; corriger les bugs ; vérifier le fonctionnement sous Windows.
- PHASE 10 — LIVRAISON : le code source complet ; le projet compilable ; l'application Windows ; un installateur Windows si possible ; une documentation d'installation ; une documentation utilisateur ; les tests ; les instructions de mise à jour.

## 15. RÈGLES ABSOLUES DU PROJET

1. Rust est obligatoire pour les moteurs mathématiques et les simulations.
2. Aucun calcul mathématique ne doit être inventé.
3. Aucune statistique manquante ne doit être présentée comme réelle.
4. Les données du marché doivent être datées et traçables.
5. Les calculs théoriques doivent être séparés des simulations.
6. Les simulations doivent être reproductibles.
7. Les événements rares doivent être vérifiés par des méthodes adaptées.
8. Le logiciel doit fonctionner localement.
9. La consommation CPU et RAM doit être maîtrisée.
10. La base de données doit être évolutive.
11. L'architecture doit permettre l'ajout de nouveaux providers.
12. Les fonctionnalités doivent être réellement opérationnelles.
13. Les erreurs doivent être signalées clairement.
14. Les modèles doivent être vérifiables indépendamment.
15. Aucune validation mathématique ne doit être accordée sans preuves suffisantes.

Ne pas chercher à produire rapidement une application visuellement impressionnante mais techniquement vide.
La priorité est la fiabilité mathématique.

## 16. PREMIÈRE MISSION

Commence maintenant. Avant de développer :

1. Analyse tous les documents disponibles.
2. Explore méthodiquement StakeCruncher.
3. Identifie précisément les données récupérables.
4. Identifie les limites de collecte.
5. Définis l'architecture technique.
6. Établis une feuille de route de développement.

Ensuite, commence immédiatement à développer la première version fonctionnelle. Ne t'arrête pas à la présentation du plan.
Si une fonctionnalité dépend d'une donnée inaccessible, développe une solution de remplacement clairement identifiée, sans inventer les données.
Si une information technique manque, prends une décision raisonnable, documente-la et poursuis le développement lorsque cela est possible.
Travaille par étapes, compile et teste régulièrement.

**VISION FINALE :** Slot Math Intelligence doit devenir notre laboratoire central de création de slots.
Nous devons pouvoir ouvrir l'application, choisir un multiplicateur maximum comme x500, x1 000, x5 000, x10 000 ou x50 000, définir le type de jeu et ses caractéristiques, analyser le marché, générer des mathématiques, simuler des millions de spins, vérifier les résultats et obtenir un prompt complet pour Claude.
Chaque nouveau projet doit pouvoir profiter des connaissances accumulées dans les projets précédents.
Nous ne construisons pas un outil temporaire.
Nous construisons une infrastructure mathématique évolutive destinée à accompagner l'ensemble de nos futurs développements de slots.

---

## ANNEXE — AUTONOMIE : PRÉPARATION DE L'ENVIRONNEMENT

Tu es responsable de préparer intégralement ton environnement de travail.
Ne demande pas à l'utilisateur de créer manuellement les dossiers ou les fichiers.

Tu dois automatiquement :

- Créer le dossier principal SLOT-MATH-INTELLIGENCE.
- Créer toute l'arborescence nécessaire au projet.
- Créer et enregistrer les fichiers de documentation.
- Organiser les documents PDF, screenshots et autres ressources disponibles.
- Initialiser le projet Rust, Tauri et React.
- Configurer les dépendances nécessaires.
- Vérifier l'intégrité de la structure créée.

Tu dois également créer un fichier MASTER_PROMPT.md contenant les spécifications complètes du projet afin de conserver une référence permanente.
Si de nouveaux documents sont ajoutés ultérieurement, tu dois pouvoir les intégrer à la bibliothèque sans perturber le fonctionnement existant.
L'utilisateur ne doit pas avoir à gérer manuellement l'organisation technique du projet.
Prends les initiatives techniques nécessaires, documente tes décisions et avance de manière autonome.
