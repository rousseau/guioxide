#!/usr/bin/env bash
# Télécharge le jeu de test fantôme de guioxide, vérifie sa somme SHA-256 et le dézippe.
#
# Usage : telecharger.sh [dossier]      (défaut : ~/.cache/guioxide/jeu-test)
# Résultat : <dossier>/jeu (racine BIDS : piles + derivatives de toute la chaîne) et <dossier>/verite (poses et masques vrais).
# Variables : GUIOXIDE_JEU_URL (défaut : le dépôt Hugging Face du jeu, voir ci-dessous), GUIOXIDE_JEU_SHA256.
#
# Le jeu est construit par `construire.sh` à partir de l'atlas CINeMA dHCP fœtal (CC BY 4.0, Zenodo 17023473) : aucune
# donnée de patiente. Il n'est jamais dans git.
set -euo pipefail
ARCHIVE=fantome-guioxide-v1.tar.gz
URL=${GUIOXIDE_JEU_URL:-https://huggingface.co/rousseau/guioxide-jeu-test/resolve/main/$ARCHIVE}
SHA256=${GUIOXIDE_JEU_SHA256:-e27686b818d8e811e18d47e58c2ef11fc2071717384706d4841fa8f5124e1273}
DOSSIER=${1:-$HOME/.cache/guioxide/jeu-test}
mkdir -p "$DOSSIER"
if [ -d "$DOSSIER/jeu" ] && [ -f "$DOSSIER/.sha256" ] && [ "$(cat "$DOSSIER/.sha256")" = "$SHA256" ]; then
  echo "jeu déjà présent : $DOSSIER/jeu"
  exit 0
fi
curl -fsSL -o "$DOSSIER/$ARCHIVE" "$URL"
echo "$SHA256  $DOSSIER/$ARCHIVE" | sha256sum -c -
rm -rf "$DOSSIER/jeu" "$DOSSIER/verite"
tar xzf "$DOSSIER/$ARCHIVE" -C "$DOSSIER"
echo "$SHA256" > "$DOSSIER/.sha256"
rm -f "$DOSSIER/$ARCHIVE"
echo "jeu prêt : $DOSSIER/jeu"
