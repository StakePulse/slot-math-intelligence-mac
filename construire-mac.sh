#!/bin/bash
# Fabrique Slot Math Intelligence pour macOS, SUR UN MAC (Apple Silicon ou Intel).
# Usage, dans le Terminal, depuis ce dossier :
#   bash construire-mac.sh            édition à partager (marché daté + modèle d'exemple)
#   bash construire-mac.sh complet    édition complète (+ FINDERS KEEPERS, documents, PROJECT)
# Résultat : LIVRAISON/<édition>/Slot Math Intelligence_<version>_universal.dmg (+ l'outil smi).
set -euo pipefail
cd "$(dirname "$0")"
ICI="$(pwd)"
EDITION="${1:-partage}"
case "$EDITION" in
  partage) EXTRA=() ;;
  complet) EXTRA=(--config src-tauri/tauri.complet.conf.json) ;;
  *) echo "Édition inconnue : $EDITION (partage ou complet)"; exit 1 ;;
esac

manque() { echo; echo "MANQUANT : $1"; echo "  → $2"; exit 1; }

xcode-select -p >/dev/null 2>&1 || manque "outils de développement d'Apple" "lancez : xcode-select --install   (puis relancez ce script)"
command -v cargo >/dev/null 2>&1 || manque "Rust" "installez-le depuis https://rustup.rs (une commande à coller dans le Terminal), puis rouvrez le Terminal"
command -v npm >/dev/null 2>&1 || manque "Node.js 20 ou plus" "installez la version LTS depuis https://nodejs.org"

echo "== Cibles Rust (Apple Silicon + Intel)"
rustup target add aarch64-apple-darwin x86_64-apple-darwin

cd "$ICI/SOURCE/app"
echo "== Interface"
npm ci --no-audit --no-fund
npm run build

cd "$ICI/SOURCE"
echo "== Tests du moteur et des données"
cargo test --release -p smi-core -p smi-data
echo "== Test de toutes les commandes de l'interface (sans fenêtre)"
cargo test -p slot-math-intelligence

echo "== Application universelle, édition $EDITION"
(cd app && npx tauri build --target universal-apple-darwin --bundles app,dmg ${EXTRA[@]+"${EXTRA[@]}"})

echo "== Outil en ligne de commande smi"
cargo build --release -p smi-cli --target aarch64-apple-darwin
cargo build --release -p smi-cli --target x86_64-apple-darwin

OUT="$ICI/LIVRAISON/$EDITION"
mkdir -p "$OUT"
cp target/universal-apple-darwin/release/bundle/dmg/*.dmg "$OUT/"
lipo -create -output "$OUT/smi" target/aarch64-apple-darwin/release/smi target/x86_64-apple-darwin/release/smi

echo
echo "Fini. Dans $OUT :"
ls -lh "$OUT"
echo "Ouvrez le .dmg et glissez l'application dans Applications (voir DOCS/INSTALLATION_MAC.md)."
