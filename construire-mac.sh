#!/bin/bash
# Fabrique Slot Math Intelligence pour macOS, SUR UN MAC (Apple Silicon ou Intel).
# Usage, dans le Terminal, depuis ce dossier :   bash construire-mac.sh
# Résultat : LIVRAISON/Slot Math Intelligence_0.1.0_universal.dmg (+ l'outil en ligne de commande smi).
set -euo pipefail
cd "$(dirname "$0")"
ICI="$(pwd)"

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

echo "== Application universelle"
(cd app && npx tauri build --target universal-apple-darwin)

echo "== Outil en ligne de commande smi"
cargo build --release -p smi-cli --target aarch64-apple-darwin
cargo build --release -p smi-cli --target x86_64-apple-darwin

mkdir -p "$ICI/LIVRAISON"
cp target/universal-apple-darwin/release/bundle/dmg/*.dmg "$ICI/LIVRAISON/"
lipo -create -output "$ICI/LIVRAISON/smi" target/aarch64-apple-darwin/release/smi target/x86_64-apple-darwin/release/smi

echo
echo "Fini. Dans $ICI/LIVRAISON :"
ls -lh "$ICI/LIVRAISON"
echo "Ouvrez le .dmg et glissez l'application dans Applications (voir DOCS/INSTALLATION_MAC.md)."
