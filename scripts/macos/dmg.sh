#!/usr/bin/env bash
# Construit guioxide.app et une image disque guioxide-<version>.dmg (macOS seulement).
#
#   bash scripts/macos/dmg.sh            # → target/dmg/guioxide-<version>.dmg
#
# Ouvrir le .dmg et glisser guioxide dans Applications, puis l'ajouter au Dock. Signature ad hoc (pas de compte
# développeur Apple) : sur la machine qui l'a construit, l'application s'ouvre directement ; copiée sur un autre Mac,
# elle demande un clic droit → Ouvrir la première fois.
set -euo pipefail

[[ "$(uname -s)" == Darwin ]] || { echo "dmg.sh : macOS seulement" >&2; exit 1; }
racine="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$racine"

version="$(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="guioxide"))')"
commit="$(git rev-parse --short HEAD)"
cible="${CARGO_TARGET_DIR:-$racine/target}"
sortie="$cible/dmg"
app="$sortie/guioxide.app"

echo "guioxide $version ($commit) : compilation"
cargo build --release -p guioxide

rm -rf "$sortie" && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$cible/release/guioxide" "$app/Contents/MacOS/guioxide"

# Icône : toutes les tailles de l'iconset macOS depuis le PNG 1024 x 1024.
iconset="$sortie/guioxide.iconset"
mkdir -p "$iconset"
for t in 16 32 128 256 512; do
  sips -z $t $t crates/guioxide/assets/icone.png --out "$iconset/icon_${t}x${t}.png" >/dev/null
  sips -z $((2 * t)) $((2 * t)) crates/guioxide/assets/icone.png --out "$iconset/icon_${t}x${t}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/guioxide.icns"
rm -rf "$iconset"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>guioxide</string>
  <key>CFBundleDisplayName</key><string>guioxide</string>
  <key>CFBundleIdentifier</key><string>io.github.rousseau.guioxide</string>
  <key>CFBundleExecutable</key><string>guioxide</string>
  <key>CFBundleIconFile</key><string>guioxide</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version+$commit</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.medical</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Licence MIT</string>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$app"

# Image disque : l'application et un lien vers /Applications, à côté.
dossier="$sortie/contenu"
mkdir -p "$dossier"
cp -R "$app" "$dossier/"
ln -s /Applications "$dossier/Applications"
dmg="$sortie/guioxide-$version.dmg"
hdiutil create -volname "guioxide $version" -srcfolder "$dossier" -fs HFS+ -format UDZO -ov "$dmg" >/dev/null 2>&1
rm -rf "$dossier"
echo "$dmg"
