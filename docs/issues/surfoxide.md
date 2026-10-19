# Issues à ouvrir sur surfoxide

Contexte commun à coller en tête de chaque issue :

> guioxide (https://github.com/rousseau/guioxide) visualise les sorties de la chaîne
> medoxide → surfoxide → gyroxide. Le contrat de données est dans `guioxide/docs/donnees.md`.

---

## 1. Lecture/écriture GIfTI dans `surfoxide-core`

Le README fixe GIfTI pour les surfaces, mais aucun code ne lit ni n'écrit ce format.

**Proposition** : un module `gifti` dans `surfoxide-core` (qui reste sans Burn) :

- lecture : `POINTSET` (float32) → sommets, `TRIANGLE` (int32) → faces, données par sommet
  (`.shape.gii`) ; encodages `ASCII`, `Base64Binary`, `GZipBase64Binary` ; transformation
  éventuelle `CoordinateSystemTransformMatrix` ;
- écriture : `GZipBase64Binary`, métadonnée `AnatomicalStructurePrimary` (`CortexLeft` /
  `CortexRight`), repère RAS mm ;
- tests aller-retour, et lecture d'un fichier produit par un outil de référence
  (FreeSurfer `mris_convert` ou nibabel).

**Pourquoi dans `surfoxide-core`** : gyroxide (surfaces simulées) et guioxide (affichage) en
ont besoin. Un seul lecteur évite trois interprétations différentes du format.

---

## 2. Lecture/écriture `.vtu`

Même logique pour les maillages volumiques (entrée de gyroxide) : VTK XML `UnstructuredGrid`,
tétraèdres (`VTK_TETRA` = 10), données par point.

---

## 3. Sorties en dérivées BIDS

**Proposition** : `surfx` écrit dans `<sortie>/derivatives/surfx-surfaces/sub-X/ses-Y/anat/` :

- `sub-X_ses-Y_hemi-{L,R}_{white,pial,midthickness}.surf.gii`
- `sub-X_ses-Y_hemi-{L,R}_{curv,sulc,thickness}.shape.gii`
- `sub-X_ses-Y_hemi-{L,R}_desc-<…>_tet.vtu`
- `sub-X_ses-Y_desc-surfqc.tsv` : `surface n_vertices n_faces euler self_intersections
  area_mm2 gi mean_dist_to_seg_mm`
- `dataset_description.json` (`GeneratedBy`)

Entrées lues depuis `derivatives/medx-svr/` et `derivatives/medx-bounti/`.

---

## 4. Toolchain, CI, backends

Voir `guioxide/docs/developpement.md` :

- même version de Rust épinglée dans les quatre dépôts ;
- ajouter `macos-latest` à la CI existante ;
- vérifier `surfx` avec le backend `wgpu` sur Mac (Metal), en plus de CUDA sur la station.
