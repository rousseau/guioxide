#!/usr/bin/env bash
# Construit guioxide depuis le commit courant (scripts/macos/dmg.sh) et l'installe dans /Applications, où le Dock le
# trouve : l'application de tous les jours, séparée des arbres de travail où l'on développe.
#
#   bash scripts/macos/installer.sh                    construire puis installer
#   bash scripts/macos/installer.sh --sans-construire  installer l'application déjà construite (target/dmg)
#
# Sorties : 0 installé · 1 erreur · 3 en attente (l'application installée tourne ; la nouvelle version est prête,
# relancer avec --sans-construire après l'avoir quittée). Journal : ~/Library/Logs/guioxide-installations.log.
set -euo pipefail

[[ "$(uname -s)" == Darwin ]] || { echo "installer.sh : macOS seulement" >&2; exit 1; }
cd "$(dirname "$0")/../.."
cible_cargo="${CARGO_TARGET_DIR:-$PWD/target}"
paquet="$cible_cargo/dmg/guioxide.app"
tampon="$cible_cargo/dmg/.commit"
journal="$HOME/Library/Logs/guioxide-installations.log"

if [[ "${1:-}" != "--sans-construire" ]]; then
  if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    echo "arbre de travail modifié : on n'installe qu'un commit, rien construit" >&2
    exit 1
  fi
  commit="$(git rev-parse --short HEAD)"
  bash scripts/macos/dmg.sh
  echo "$commit" > "$tampon"
fi

[[ -d "$paquet" ]] || { echo "application introuvable : $paquet (lancer sans --sans-construire)" >&2; exit 1; }
version="$(cat "$tampon" 2>/dev/null || echo inconnue)"

dest=/Applications
[[ -w "$dest" ]] || { dest="$HOME/Applications"; mkdir -p "$dest"; }
cible="$dest/guioxide.app"

if pgrep -f "$cible/Contents/MacOS/" >/dev/null 2>&1; then
  echo "en attente : guioxide installé tourne. Version $version prête ; le quitter, puis :"
  echo "  bash scripts/macos/installer.sh --sans-construire"
  exit 3
fi

rm -rf "$cible"
ditto "$paquet" "$cible"
# Icône : macOS garde l'ancienne en cache ; la date du paquet et un nouvel enregistrement la font relire.
touch "$cible"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$cible"
mkdir -p "$(dirname "$journal")"
printf '%s  %s  %s\n' "$(date -u +%FT%RZ)" "$version" "$cible" >> "$journal"
echo "installé : guioxide $version dans $cible"
