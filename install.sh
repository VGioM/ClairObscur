#!/bin/bash
# Installs ClairObscur.app into /Applications.
set -euo pipefail
cd "$(dirname "$0")"

./build.sh

echo "▸ Installation dans /Applications…"
rm -rf "/Applications/ClairObscur.app"
cp -R dist/ClairObscur.app "/Applications/ClairObscur.app"

echo "✓ Installé. Ouvrez-le depuis le Launchpad ou :"
echo "  open /Applications/ClairObscur.app"
