# Issues à ouvrir sur medoxide

Contexte commun à coller en tête de chaque issue :

> guioxide (https://github.com/rousseau/guioxide) visualise les sorties de la chaîne
> medoxide → surfoxide → gyroxide. Le contrat de données est dans `guioxide/docs/donnees.md`.

---

## 1. Affine : règle unique `sform`, sinon `qform`, dans `medoxide-core`

> **Ouverte** : [medoxide#1](https://github.com/rousseau/medoxide/issues/1), périmètre réduit. La
> règle existe désormais dans `medoxide-core` (`affine_nifti`, utilisée par guioxide et par
> medoxide-bounti) ; reste à l'utiliser dans medoxide-svr et medoxide-fetalbet.

`medoxide-core::volume_info` ne lit que le `sform` et renvoie `affine: None` si
`sform_code == 0`. Certaines piles fœtales n'ont qu'un `qform`.

**Proposition**
- Ajouter dans `medoxide-core` une fonction `affine(header) -> Affine` : `sform` si
  `sform_code > 0`, sinon `qform` (quaternion + `pixdim` + `qfac`) si `qform_code > 0`,
  sinon diagonale des `pixdim`, avec l'origine de l'en-tête, et un indicateur de la source.
- Utiliser cette fonction dans `medoxide-svr` (géométrie des piles) et dans guioxide.
- Tests : en-têtes `sform` seul, `qform` seul, les deux en désaccord, aucun.

**Pourquoi** : si l'interface et le SVR n'utilisent pas la même affine, les coupes affichées
ne coïncident pas avec le volume reconstruit.

---

## 2. Sorties du SVR : chemins BIDS et noms fixes

Aujourd'hui `medx svr recon` écrit où l'appelant le demande (`--output`, `--poses`).

**Proposition** : une option `--bids <racine> --participant X --session Y [--output-dir <sortie>]`
qui écrit dans `<sortie>/derivatives/medx-svr/sub-X/ses-Y/anat/` :

- `sub-X_ses-Y_desc-svr_T2w.nii.gz`
- `sub-X_ses-Y_desc-svr_T2w.json` (résolution, mode de mouvement, itérations, piles utilisées)
- `sub-X_ses-Y_desc-svr_motion.tsv`
- `sub-X_ses-Y_desc-svr_slicequality.tsv`

et `derivatives/medx-svr/dataset_description.json` (`GeneratedBy` : nom, version, commit).
Les options actuelles restent pour l'usage hors BIDS. Même logique pour `medx fetalbet`
(dont le nom `desc-brain_mask` existe déjà).

---

## 3. Fichier de mouvement : en-tête et nom de pile

`SvrOutput::write_poses` écrit `indice_de_pile \t coupe \t` + 16 valeurs (ordre colonne),
**sans en-tête**, l'indice étant la position de la pile dans `--input`.

**Proposition**
- ligne d'en-tête `stack	slice	m00	m10	m20	m30	m01	…	m33` ;
- `stack` = nom du fichier de la pile (sans extension) au lieu de son indice ;
- documenter dans le `.json` voisin ce que représente la matrice (coupe → monde, ou correction
  à composer avec l'affine de la pile).

guioxide en tire translation (mm) et rotation (°) par coupe pour la courbe de mouvement.

---

## 4. Qualité par coupe : nom de pile, exclusion, motif

`write_quality` écrit déjà `stack coupe recalee score_avant score_apres nb_recalages
correction_rms_mm`.

**Proposition**
- `stack` = nom de fichier (comme pour le mouvement) ;
- colonnes ajoutées : `excluded` (0/1, coupe retirée de la reconstruction, par exemple par
  `s2s_exclude_fraction`) et `reason` ;
- en-têtes en anglais, alignés sur le contrat : `stack slice registered score_before
  score_after n_registrations correction_rms_mm excluded reason`.

Question ouverte : medoxide calculera-t-il aussi un QC **avant** reconstruction (artefacts,
signal, couverture, par pile et par coupe) ? Si oui, même format, `desc-acq_slicequality.tsv`.

---

## 5. Segmentation : nom et table d'étiquettes

Pour `medoxide-bounti` :

- `derivatives/medx-bounti/sub-X/ses-Y/anat/sub-X_ses-Y_desc-bounti_dseg.nii.gz`, sur la
  grille du volume SVR ;
- `sub-X_ses-Y_desc-bounti_dseg.tsv` (format BIDS) : `index name abbreviation color`.

---

## 6. Toolchain, CI, backends

Voir `guioxide/docs/developpement.md` :

- épingler la même version de Rust que surfoxide et gyroxide ;
- CI `ubuntu-latest` + `macos-latest` (fmt, clippy, test sur CPU) ;
- vérifier que `medx` tourne sur la station RTX 2080 Ti (Vulkan, ou CUDA via une feature).
