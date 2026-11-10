#!/usr/bin/env bash
# Construit le jeu de test fantôme de guioxide (docs/donnees.md) : trois piles T2w simulées avec un mouvement connu à partir
# de l'atlas CINeMA dHCP fœtal (CC BY 4.0, Zenodo 17023473), puis la vraie chaîne en mode --bids.
#
# Usage : construire.sh <dossier de travail>
# Variables : ATLAS (dossier de dhcp_fetal.tar.gz dézippé ; défaut : le télécharge dans <travail>/atlas), GA (32),
#   MEDX, MEDX_SIMULATE (binaire dont `svr simulate` existe : medoxide f4/0.2-juge ou plus récent), SURFX, GYROXIDE,
#   PAS_GYROXIDE (pas du run court, 200 ; CPU de référence, ndarray f64), TETGEN (trouvé tout seul par gyroxide sinon).
# Sortie : <travail>/jeu (racine BIDS : piles + derivatives/), <travail>/verite (poses vraies, masques vrais).
set -euo pipefail
TRAVAIL=${1:?usage : construire.sh <dossier de travail>}
ICI=$(cd "$(dirname "$0")" && pwd)
GA=${GA:-32}
MEDX=${MEDX:-medx}
MEDX_SIMULATE=${MEDX_SIMULATE:-$MEDX}
SURFX=${SURFX:-surfx}
GYROXIDE=${GYROXIDE:-gyroxide}
PAS_GYROXIDE=${PAS_GYROXIDE:-200}
SUJET=fant01
SESSION=01
mkdir -p "$TRAVAIL"
TRAVAIL=$(cd "$TRAVAIL" && pwd)
chrono() { local debut=$SECONDS; "$@"; echo "  ($((SECONDS - debut)) s)"; }

if [ -z "${ATLAS:-}" ]; then
  ATLAS="$TRAVAIL/atlas"
  if [ ! -f "$ATLAS/dhcp_fetal/T2w_ga=21-37.nii.gz" ]; then
    mkdir -p "$ATLAS"
    curl -fsSL -o "$ATLAS/dhcp_fetal.tar.gz" "https://zenodo.org/api/records/17023473/files/dhcp_fetal.tar.gz/content"
    echo "59f8d09d4291682d3368b3b1d232b880  $ATLAS/dhcp_fetal.tar.gz" | md5sum -c -
    tar xzf "$ATLAS/dhcp_fetal.tar.gz" -C "$ATLAS"
  fi
  ATLAS="$ATLAS/dhcp_fetal"
fi

echo "1. source et gabarits"
python3 -I "$ICI/preparer_source.py" --atlas "$ATLAS" --ga "$GA" --sortie "$TRAVAIL/source"

echo "2. piles simulées (mouvement lisse ±3 °/mm, bruit 1 %, biais 10 %, échelle de coupe 3 %, graine 1)"
rm -rf "$TRAVAIL/piles" "$TRAVAIL/jeu" "$TRAVAIL/verite"
"$MEDX_SIMULATE" svr simulate --source "$TRAVAIL/source/source_T2w.nii.gz" --source-mask "$TRAVAIL/source/source_mask.nii.gz" \
  --template "$TRAVAIL"/source/gabarit-{axial,coronal,sagittal}.nii.gz --output-dir "$TRAVAIL/piles" --name sim \
  --motion smooth --amplitude 3 --interleaved --noise-pct 1 --bias-pct 10 --slice-scale-pct 3 --seed 1

echo "3. jeu BIDS"
ANAT="$TRAVAIL/jeu/sub-$SUJET/ses-$SESSION/anat"
mkdir -p "$ANAT" "$TRAVAIL/verite"
i=0
for o in ax cor sag; do
  i=$((i + 1))
  cp "$TRAVAIL/piles/sim_acq-${i}_T2w.nii.gz" "$ANAT/sub-${SUJET}_ses-${SESSION}_acq-sim${o}_run-1_T2w.nii.gz"
  cp "$TRAVAIL/piles/sim_acq-${i}_T2w.json" "$ANAT/sub-${SUJET}_ses-${SESSION}_acq-sim${o}_run-1_T2w.json"
  cp "$TRAVAIL/piles/sim_acq-${i}_mask.nii.gz" "$TRAVAIL/verite/"
done
cp "$TRAVAIL/piles/sim_poses_vraies.tsv" "$TRAVAIL/verite/"
cat > "$TRAVAIL/jeu/dataset_description.json" <<JSON
{
  "Name": "Fantôme numérique guioxide (3 piles T2w simulées, mouvement connu)",
  "BIDSVersion": "1.10.0",
  "DatasetType": "raw",
  "License": "CC BY 4.0 (atlas CINeMA dHCP fœtal, Zenodo 17023473, Dannecker et al. 2025)"
}
JSON
printf 'participant_id\nsub-%s\n' "$SUJET" > "$TRAVAIL/jeu/participants.tsv"
printf 'session_id\tga_weeks\nses-%s\t%s\n' "$SESSION" "$GA.0" > "$TRAVAIL/jeu/sub-$SUJET/sub-${SUJET}_sessions.tsv"

OPT=(--bids "$TRAVAIL/jeu" --participant "$SUJET" --session "$SESSION")
echo "4. medx fetalbet";    chrono "$MEDX" fetalbet "${OPT[@]}"
echo "5. medx svr qc";      chrono "$MEDX" svr qc "${OPT[@]}"
echo "6. medx svr recon";   chrono "$MEDX" svr recon "${OPT[@]}" --device cpu
echo "7. medx bounti";      chrono "$MEDX" bounti "${OPT[@]}" --device cpu
echo "8. surfx run";        chrono "$SURFX" run "${OPT[@]}"
echo "9. gyroxide mesh";    chrono "$GYROXIDE" mesh "${OPT[@]}" --hemi L
echo "10. gyroxide run ($PAS_GYROXIDE pas)"; chrono "$GYROXIDE" run "${OPT[@]}" --hemi L --label court --steps "$PAS_GYROXIDE" --backend ndarray
echo "jeu construit : $TRAVAIL/jeu"
find "$TRAVAIL/jeu/derivatives" -name dataset_description.json | sort
