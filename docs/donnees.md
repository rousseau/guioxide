# Contrat de données

Ce document fixe **ce que chaque outil écrit et ce que guioxide lit** : chemins, noms,
formats, repères. C'est le seul couplage entre les quatre dépôts. Une modification ici se
fait par PR, relue côté outil et côté guioxide.

Statut de chaque ligne : **existant** (déjà écrit ainsi par l'outil), **à adapter** (l'outil
écrit autre chose aujourd'hui), **proposé** (n'existe pas encore).

## Règles communes

- **Jeu de données** : BIDS. Les données brutes sous `sub-*/ses-*/anat/` (et `dwi/` plus tard).
- **Racine de l'étude** : la racine BIDS elle-même, ou son parent quand la racine BIDS s'appelle
  `sourcedata/` (convention des buckets Garage : `sourcedata/` et `derivatives/` côte à côte).
- **Dérivées** : `<sortie>/derivatives/<pipeline>/sub-*/ses-*/<modalité>/`, où `<sortie>` est
  la racine de l'étude (mode « étude ») ou un répertoire libre (mode « test »).
  `<pipeline>` = `<binaire>-<méthode>` : `medx-fetalbet`, `medx-svr`, `medx-bounti`,
  `surfx-surfaces`, `gyroxide`.
- **Chaque dossier `<pipeline>/` contient un `dataset_description.json`** avec
  `DatasetType: "derivative"` et `GeneratedBy` (nom, version, et si possible commit et ligne
  de commande). guioxide en lit la version pour l'afficher dans la barre de provenance.
  *Statut : proposé pour les trois outils.*
- **Noms de fichiers** : on garde les entités de la source (`sub`, `ses`, `acq`, `run`) et on
  ajoute `desc-<…>` pour décrire la dérivée.
- **Images** : NIfTI (`.nii.gz`). Affine : `sform`, sinon `qform` (voir architecture).
- **Surfaces** : GIfTI (`.surf.gii`, `.shape.gii`). **Maillages volumiques** : VTK `.vtu`.
- **Repère des maillages** : monde RAS, mm, celui de l'image source. Faces orientées normales
  sortantes. Jamais de coordonnées normalisées dans un fichier de sortie.
- **Tableaux** : TSV avec ligne d'en-tête, séparateur tabulation, décimal `.`, `n/a` pour une
  valeur absente. Chaque fichier TSV a idéalement un `.json` voisin décrivant ses colonnes.
- **Indices de pile** : une pile est désignée par son **nom de fichier source** (sans
  extension), pas par sa position dans une liste d'arguments.

## 0 · Jeu BIDS (lu par guioxide-bids)

| Fichier | Usage | Statut |
|---|---|---|
| `dataset_description.json` | reconnaissance de la racine, nom du jeu, version BIDS | BIDS |
| `participants.tsv` | liste des sujets | BIDS |
| `sub-*/sub-*_sessions.tsv` (colonne `session_id`), ou `participants.tsv` pour un jeu sans sessions : colonne `ga_weeks` | affichage AG (SA) | **existant** (`fetal-crl2025`) ; même nom que dans `sequence.tsv` de gyroxide |
| `sub-*/ses-*/anat/*_T2w.nii.gz` | piles acquises (une par fichier, `acq-`/`run-` pour les distinguer) | BIDS |
| `derivatives/*/dataset_description.json` | dérivées présentes, versions | proposé |

Si l'utilisateur dépose un dossier `sub-*`, `ses-*` ou plus bas, guioxide remonte jusqu'au premier
parent contenant un `dataset_description.json` de jeu brut (ceux des dérivées, `DatasetType:
derivative`, sont ignorés). S'il dépose la racine d'un bucket, la racine BIDS est `sourcedata/`.

## 1 · Examen — medoxide

| Fichier | Contenu | Statut |
|---|---|---|
| `medx-fetalbet/sub-X/ses-Y/anat/<entités de la pile, sans son suffixe>_desc-brain_mask.nii.gz` | masque cérébral d'une pile, même grille que la pile | **existant** (ex. `sub-X_ses-Y_acq-ax_run-1_T2w` → `sub-X_ses-Y_acq-ax_run-1_desc-brain_mask.nii.gz`) |
| `medx-svr/sub-X/ses-Y/anat/sub-X_ses-Y_desc-svr_slicequality.tsv` | qualité par coupe (voir ci-dessous) ; motifs d'exclusion dont `hors_masque` | **existant** (medoxide `f2fbcb6`, `medx … --bids` ; lu par `medoxide_core::sorties`) |

Colonnes de `slicequality.tsv` (reprend `write_quality` de `medoxide-svr`, en ajoutant le nom
de pile et le motif d'exclusion) :

| Colonne | Type | Sens |
|---|---|---|
| `stack` | texte | nom du fichier source de la pile, sans extension |
| `slice` | entier | indice de coupe dans la pile (0 = première) |
| `registered` | 0/1 | coupe recalée |
| `score_before`, `score_after` | réel ∈ [−1, 1] | concordance (s2s) ou corrélation à la référence (s2v) |
| `n_registrations` | entier | nombre de recalages |
| `correction_rms_mm` | réel | amplitude de la correction de mouvement |
| `excluded` | 0/1 | coupe exclue de la reconstruction |
| `reason` | texte | motif d'exclusion (`outlier`, `hors_masque`, …) ou `n/a` |

> Ce fichier est un QC **après** recalage. Un QC des acquisitions **avant** reconstruction
> (artefacts, signal, couverture) est à définir dans medoxide ; s'il existe, il suivra le même
> format avec `desc-acq_slicequality.tsv`. L'onglet Examen affiche celui qui est présent.

## 2 · Reconstruction — medoxide

| Fichier | Contenu | Statut |
|---|---|---|
| `medx-svr/sub-X/ses-Y/anat/sub-X_ses-Y_desc-svr_T2w.nii.gz` | volume reconstruit (sform et qform écrits) | **existant** (medoxide `f2fbcb6`) |
| `medx-svr/…/sub-X_ses-Y_desc-svr_T2w.json` | paramètres (résolution, mode de mouvement, réglages, piles utilisées, ligne de commande, convention du mouvement) | **existant** (medoxide `f2fbcb6`) |
| `medx-svr/…/sub-X_ses-Y_desc-svr_motion.tsv` | une ligne par coupe : `stack`, `slice`, puis la matrice 4×4 | **existant** (medoxide `f2fbcb6`, `medx … --bids` ; lu par `medoxide_core::sorties`) |
| `medx-svr/…/sub-X_ses-Y_desc-svr_slicequality.tsv` | voir étape 1 | **existant** |
| `medx-bounti/sub-X/ses-Y/anat/sub-X_ses-Y_desc-bounti_dseg.nii.gz` | segmentation, même grille que le volume reconstruit | **existant** (medoxide `f2fbcb6`) |
| `medx-bounti/…/sub-X_ses-Y_desc-bounti_dseg.tsv` | table des étiquettes : `index`, `name`, `abbreviation`, `color` (19 étiquettes, couleurs de l'atlas KCL) | **existant** (medoxide `f2fbcb6`, `medx … --bids` ; lu par `medoxide_core::sorties`) |

**Mouvement** (medoxide#3, fait) :

- ligne d'en-tête : `stack slice m00 m10 m20 m30 m01 … m33` (ordre colonne) ;
- `stack` = nom de fichier de la pile, sans extension ;
- sens de la matrice documenté dans le `.json` voisin : mouvement rigide `M` de la coupe, absolu par
  rapport à la géométrie d'en-tête et composé dans le repère monde : coupe → monde = `M · A · T(0, 0, k)`.

`medoxide_core::sorties::mouvement_par_coupe` en déduit, par coupe, le déplacement du centre de la coupe `‖M·c − c‖` (mm, `c` = centre
de la coupe `k` dans la géométrie d'en-tête) et l'angle de rotation de `M` (°), pour la courbe de
mouvement de l'onglet 2.

## 3 · Surfaces — surfoxide

Convention de nommage des surfaces : celle des dérivées BIDS de surface (style fMRIPrep).

| Fichier | Contenu | Statut |
|---|---|---|
| `surfx-surfaces/sub-X/ses-Y/anat/sub-X_ses-Y_hemi-{L,R}_white.surf.gii` | surface blanche (SB/SG) | **existant** (surfoxide `dc02bf7`, `surfx run --bids`) |
| `…_hemi-{L,R}_pial.surf.gii` | surface piale (SG/LCS) | **existant** (surfoxide `dc02bf7`, `surfx run --bids`) |
| `…_hemi-{L,R}_midthickness.surf.gii` | surface médiane | proposé, optionnel |
| `…_hemi-{L,R}_{curv,sulc,thickness}.shape.gii` | cartes par sommet ; `sulc` = profondeur DPF* (méthode dans les métadonnées GIfTI) ; signe de `curv` et `sulc` : **crêtes gyrales positives, fonds de sillons négatifs** (convention de l'atlas dHCP, opposée à FreeSurfer), écrit dans la métadonnée GIfTI `SignConvention` (décidé le 22 oct. 2026) | **existant** (surfoxide `dc02bf7`, `surfx run --bids` ; signe documenté `790e75b`) |
| `…_desc-surfqc.tsv` | **le** contrôle des maillages, une ligne par surface : `surface`, `n_vertices`, `n_faces`, `euler`, `self_intersections`, `area_mm2`, `gi`, `mean_dist_to_seg_mm` ; `report.json` reste un détail facultatif (décidé le 20 oct. 2026) | **existant** (surfoxide `dc02bf7`, `surfx run --bids`), lu par `surfoxide_io::surfqc` |

`hemi-` peut être omis si la surface couvre les deux hémisphères. GIfTI : intent
`NIFTI_INTENT_POINTSET` en `float32` pour les sommets, `NIFTI_INTENT_TRIANGLE` en `int32` pour
les faces, encodage `GZipBase64Binary`. La métadonnée `AnatomicalStructurePrimary`
(`CortexLeft` / `CortexRight`) est renseignée.

Le maillage tétraédrique (état initial de gyroxide) n'est **pas** produit par surfoxide : voir la section 4
(décidé le 22 oct. 2026).

## 4 · Simulation — gyroxide

Une simulation = un *run*, identifié par un label court.

```
gyroxide/sub-X/ses-Y/anat/run-<label>/
  config.toml                         paramètres complets (préréglage + surcharges)
  status.json                         état du calcul (voir ci-dessous)
  sequence.tsv                        une ligne par sortie : step, time, file, gi, energy, n_contacts
  sub-X_ses-Y_hemi-L_desc-sim_step-0000000_pial.surf.gii
  sub-X_ses-Y_hemi-L_desc-sim_step-0005000_pial.surf.gii
  …
```

| Élément | Contenu | Statut |
|---|---|---|
| surfaces de la séquence | GIfTI, **repère monde RAS mm** (transformée inverse de la normalisation appliquée) | **à adapter** : gyroxide écrit aujourd'hui du VTK legacy ASCII, en coordonnées normalisées |
| `sequence.tsv` | index de la séquence ; `time` dans l'unité du modèle ; `ga_weeks` interpolé depuis le paramètre explicite `schedule.ga_weeks` du préréglage (même longueur que `schedule.t`), `n/a` sinon ; `gi` = aire de la surface / aire de son enveloppe convexe ; `energy` = énergie élastique `Ue` (décidé le 20 oct. 2026) | proposé |
| `status.json` | `{"state": "running" \| "done" \| "failed", "step": n, "total_steps": N, "started": "<ISO 8601>", "updated": "<ISO 8601>", "message": "…"}` | proposé |
| `config.toml` | paramètres (le format TOML de `gyroxide-model` existe déjà) | **existant** (à écrire dans le run) |
| état initial | maillage tétraédrique de la session, `derivatives/gyroxide/sub-X/ses-Y/anat/sub-X_ses-Y_hemi-{L,R}_desc-<…>_tet.vtu` (VTK `UnstructuredGrid`, mm, repère de l'image, appartenance substance grise / blanche par nœud), **produit par gyroxide** à partir de la blanche, de la piale et du masque du cortex de `surfx` ; TetGen en outil externe à court terme, puis isosurface stuffing en Rust dans `gyroxide-mesh` ; chemin relatif dans `config.toml` (décidé le 22 oct. 2026) | proposé |

Les traces NPY de validation (`--trace-every`) restent hors contrat : outil de développement,
non lu par guioxide.

## Déduction de l'état d'avancement (guioxide-bids)

| Étape | Terminée si | En cours si |
|---|---|---|
| 1 · Examen | toutes les piles `T2w` de la session ont un masque `medx-fetalbet` | une partie seulement |
| 2 · Reconstruction | `desc-svr_T2w.nii.gz` **et** `desc-bounti_dseg.nii.gz` présents | l'un des deux |
| 3 · Surfaces | `white` et `pial` présents pour les hémisphères attendus | une partie |
| 4 · Simulation | au moins un run avec `status.json` `done` | un run `running` |

Un fichier présent mais illisible marque l'étape en erreur, avec le message de lecture.

## Validation par l'opérateur — guioxide

guioxide n'écrit que ses propres annotations, jamais dans les dérivées des outils.

```
<sortie>/derivatives/guioxide/
  dataset_description.json            GeneratedBy : guioxide et sa version
  sub-X/ses-Y/sub-X_ses-Y_desc-qc.json
```

`desc-qc.json` : une entrée par étape (`examen`, `reconstruction`, `surfaces`, `simulation`), chacune
`{"etat": "valide" | "a_revoir" | "rejete", "note": "…", "operateur": "…", "date": "<ISO 8601>",
"outil": "<nom version>"}`, `outil` étant lu dans le `GeneratedBy` de la dérivée validée. Une étape
absente n'est pas encore évaluée. (Décidé le 20 oct. 2026.)

## Évolutions prévues

- **Multi-contraste** : l'entité de suffixe (`T2w`, `T1w`, …) est conservée dans tous les
  noms ; guioxide liste les contrastes présents et les propose dans l'en-tête.
- **Diffusion** : données sous `dwi/` (`*_dwi.nii.gz`, `.bval`, `.bvec`) ; dérivées à
  définir (`desc-<…>_tensor`, cartes FA/MD) quand medoxide les produira.
