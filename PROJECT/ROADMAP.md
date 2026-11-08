# Feuille de route

État au 2026-11-06 (version 0.1.0).

| Phase | État | Livré |
|---|---|---|
| 1 — Audit | FAIT | `AUDIT.md` : documents (Witchcraft 85 p., Temple Drop 18 p., rapport FK, captures, slotlab), StakeCruncher (API, robots.txt, sitemaps), environnement, risques |
| 2 — Fondations | FAIT | arborescence, workspace Rust, Tauri 2, React/TS, SQLite versionnée, installateur NSIS |
| 3 — Market Intelligence | FAIT (collecte désactivée, décision du 2026-11-06) | imports (instantané tiers, lookup tables), classement bets / turnover séparés, agrégats complets, providers, sources datées ; collecteur StakeCruncher codé, désactivé (D-03) |
| 4 — Math Engine | FAIT | lois exactes en entiers, 30+ indicateurs, classes, bandes, percentiles, seuils, CVaR/ETL, barèmes 2★/3★, clôture, volatilité composite ; moteur de règles (ways/lignes, scatters, tours gratuits) avec formule fermée = énumération |
| 5 — Spin Simulator | FAIT | alias O(1), lots à graine, reproductible quel que soit le nombre de threads (~50–100 M spins/s sur 16 cœurs), pause / annulation, z / Wilson / khi-deux / Poisson, convergence, sessions, bankroll, comparaison |
| 6 — Math Generator | FAIT | loi à 4 classes atteignant exactement RTP, hit, bonus, part bonus, max win ; volatilité visée par le plancher du bonus, plancher ≥ 10× prix ; pairs du marché |
| 7 — Prompt Generator | FAIT | cahier des charges complet, exports MD/TXT/JSON + lookup table + rapport de validation |
| 8 — Document Library | FAIT (sans OCR) | import, SHA-256, rangement, pdftotext / pdf-extract, Excel, valeurs « À VALIDER », saisie manuelle, recherche |
| 9 — Tests | FAIT | 34 + 2 (non-régression FK) + 9 + 1 (IPC complet sans fenêtre) |
| 10 — Livraison | FAIT | sources, installateur, docs d'installation, d'utilisation, de mise à jour |

## Prochaines étapes proposées
1. ~~Décision sur la collecte StakeCruncher~~ — **tranchée le 2026-11-06 : désactivée** (D-03). Le marché
   s'alimente par imports ; un accès autorisé par l'éditeur du site reste la seule voie pour l'automatiser.
2. **Moteur de règles v2** : cascades (tumbles), cluster pays, Megaways, Hold & Win, multiplicateurs de wild,
   achats multiples ; puis « règles → books → lookup table » pour boucler avec la loi cible.
3. **Générateur v2** : familles de lois à deux paramètres par classe (corps + queue) pour atteindre des
   volatilités plus hautes sans rendre les bonus trop rares ; modes multiples d'un même jeu (base, ante,
   achats) générés ensemble.
4. **Rapport PDF** du modèle sur le gabarit Witchcraft (chapitres par mode).
5. **OCR local** (Tesseract) pour les captures, toujours avec validation humaine.
6. Historique des collectes en graphique (évolution des rangs et des volumes).

## Version macOS (2026-11-06)
Dossier `Desktop/SLOT-MATH-INTELLIGENCE-MAC` : même application, code rendu portable (D-15), pack de départ
embarqué, fabrication du `.dmg` par GitHub Actions ou `construire-mac.sh`. Testée sous Windows (mêmes tests +
pack de départ) ; la compilation et les tests SUR macOS se font dans l'action GitHub.

## 0.3.0 — Contrôles de conception (2026-11-08)
Slot Explorer → « Contrôles de conception » : lecture d'un dossier publié (books) et 9 contrôles issus des retours
d'un concepteur (D-17) ; règles reprises dans le prompt et la validation. À suivre : confirmer les seuils avec le
concepteur ; ajouter les bandes de rouleaux (fichier `strips`) quand un jeu les publie.
