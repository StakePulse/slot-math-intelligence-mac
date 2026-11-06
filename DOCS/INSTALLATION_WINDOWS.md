# Installation sur Windows

Windows 10 ou 11, 64 bits. Aucun droit administrateur nécessaire.

## Installer

1. Lancer `Slot Math Intelligence_0.2.0_x64-setup.exe`.
2. L'installateur n'est pas signé par un éditeur reconnu : Windows affiche **« Windows a protégé votre
   ordinateur »**. Cliquer **Informations complémentaires**, puis **Exécuter quand même**.
3. Suivre l'installateur (installation pour votre compte seulement). Si WebView2 manque (rare sur
   Windows 10, toujours présent sur Windows 11), il est téléchargé automatiquement.
4. Ouvrir **Slot Math Intelligence** depuis le menu Démarrer.

## Premier lancement

Le Dashboard affiche « installation des données de départ » pendant quelques secondes, puis :
- **la référence du marché** : instantané daté du 2026-11-03 de 125 slots Stake Engine (1 116 modes), chiffres
  marqués IMPORTÉ ;
- **un modèle d'exemple** « ×5 000 » régénéré par le moteur (statut DRAFT : le valider dans Math Engine).

## Où sont les données

`Documents\SLOT-MATH-INTELLIGENCE` (votre dossier Documents, même s'il est synchronisé par OneDrive) :

| Dossier | Contenu |
|---|---|
| `DATABASE\` | `smi.sqlite` (la base), `imports\` (sources datées), `luts\` (lookup tables importées) |
| `DOCUMENTS\` | `PDF\`, `SCREENSHOTS\`, `EXCEL\`, `OTHER\`, `INBOX\` (déposer ici, puis « Intégrer ») |
| `EXPORTS\` | `prompts\`, `reports\`, `models\`, `simulations\` |
| `DOCS\` | ce guide et le guide d'utilisation |

Settings → **Projet et intégrité** : chaque dossier a un bouton **Ouvrir**. Tout reste sur votre PC ; rien n'est envoyé.

## Des maths « propres » avec l'outil, en 5 étapes

1. **Math Generator** : choisir le max win, le RTP, le hit rate, la fréquence et la part des bonus, la
   volatilité. La loi est recalculée à chaque changement ; le panneau *Références du marché* montre les jeux
   comparables (même nature, plafond voisin) et peut appliquer leurs médianes.
2. **Enregistrer** le modèle, puis **Math Engine → Valider** : simulation reproductible, 9 étapes de
   contrôle, barèmes Stake 2★/3★. Viser le statut **VALIDATED**.
3. **Prompt Generator** : copier le cahier des charges complet pour Claude (ou l'exporter en .md).
4. Construire le jeu (rouleaux, books) avec ce cahier des charges.
5. **Le plus important** : importer la VRAIE lookup table du jeu terminé (`id,weight,payout`) dans Market
   Intelligence → « Importer une lookup table », puis l'ouvrir dans **Slot Explorer**. L'analyse est exacte
   (pas une simulation) : RTP, écart-type, max win et sa probabilité, trous dans la répartition, barèmes
   Stake, comparaison au marché. C'est ce contrôle qui évite d'envoyer une math bancale.

L'outil donne une loi cible validée et un cahier des charges ; il n'écrit ni les rouleaux ni les books.

## Désinstaller

Paramètres Windows → Applications → Slot Math Intelligence → Désinstaller. Le dossier
`Documents\SLOT-MATH-INTELLIGENCE` est conservé (à supprimer à la main si vous le souhaitez).
