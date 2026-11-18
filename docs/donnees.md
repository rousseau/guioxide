# Contrat de données

Ce document fixe **ce que chaque outil écrit et ce que guioxide lit** : chemins, noms,
formats, repères. C'est le seul couplage entre les quatre dépôts. Une modification ici se
fait par PR, relue côté outil et côté guioxide.

**Version du contrat : 1.0** (10 novembre 2026). Elle monte (1.1, 2.0…) quand un fichier « existant » change de nom,
de colonne ou de sens ; le test `crates/guioxide-bids/tests/contrat.rs` la vérifie contre ce texte et lit chaque fichier
« existant » sur le jeu de test fantôme (`scripts/jeu-test/`).

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
  *Statut : **existant** pour `medx-fetalbet`, `medx-svr`, `medx-bounti`, `surfx-surfaces` et `gyroxide` (vérifié le
  10 novembre 2026 sur le jeu fantôme : `DatasetType: derivative`, `GeneratedBy` avec `Name`, `Version`, `Commit`, `CodeURL`,
  `CommandLine`).*
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
| `derivatives/*/dataset_description.json` | dérivées présentes, versions | **existant** (medoxide `8e7d679`, surfoxide `78f0f23`, gyroxide `7ec21a2`) |

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

> Ce fichier est un QC **après** recalage. Le QC des acquisitions **avant** reconstruction est
> `medx-svr/sub-X/ses-Y/anat/sub-X_ses-Y_desc-acq_slicequality.tsv` (`medx svr qc --bids …`, medoxide
> `aa81ee9` ou plus récent), **existant**. Mêmes colonnes, avec `registered` = 0, `score_before` = concordance aux poses
> d'en-tête (`score_after` et `correction_rms_mm` à `n/a`), et trois colonnes de plus (`mask_pixels`, `mean_signal`,
> `n_partners`) que guioxide ne lit pas. `excluded` = 1 pour une coupe **signalée** ; `reason` vaut `hors_masque`,
> `masque_insuffisant`, `sans_partenaire` (le SVR ne recale pas ces trois-là) ou `signal_faible` (creux de signal). Le fichier
> `.json` voisin décrit les colonnes et les seuils.
>
> **Affichage (onglet Examen).** Un seul des deux fichiers existe : c'est lui. Les deux existent : une bascule
> « Avant reconstruction » / « Après reconstruction » (par défaut après). L'histogramme trace `score_before` (concordance
> d'en-tête) pour le contrôle, `score_after` pour la qualité après recalage ; les vignettes sont encadrées dans une couleur par
> motif et la synthèse donne le décompte par motif.

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
| surfaces de la séquence | GIfTI, **repère monde RAS mm** (transformée inverse de la normalisation appliquée) | **existant** (gyroxide `298a7ce`) |
| `sequence.tsv` | index de la séquence ; `time` dans l'unité du modèle ; `ga_weeks` interpolé depuis le paramètre explicite `schedule.ga_weeks` du préréglage (même longueur que `schedule.t`), `n/a` sinon ; `gi` = aire de la surface / aire de son enveloppe convexe ; `energy` = énergie élastique `Ue` (décidé le 20 oct. 2026) | **existant** (gyroxide `298a7ce`, `gyroxide run --bids` ; lu par `gyroxide-io`) |
| `status.json` | `{"state": "running" \| "done" \| "failed", "step": n, "total_steps": N, "started": "<ISO 8601>", "updated": "<ISO 8601>", "message": "…"}` | **existant** (gyroxide `298a7ce`, `gyroxide run --bids` ; lu par `gyroxide-io`) |
| `config.toml` | paramètres (le format TOML de `gyroxide-model` existe déjà) | **existant** (gyroxide `298a7ce`, écrit dans le run) |
| état initial | maillage tétraédrique de la session, `derivatives/gyroxide/sub-X/ses-Y/anat/sub-X_ses-Y_hemi-{L,R}_desc-<…>_tet.vtu` (VTK `UnstructuredGrid`, mm, repère de l'image, appartenance substance grise / blanche par nœud), **produit par gyroxide** à partir de la blanche, de la piale et du masque du cortex de `surfx` ; TetGen en outil externe à court terme, puis isosurface stuffing en Rust dans `gyroxide-mesh` ; chemin relatif dans `config.toml` (décidé le 22 oct. 2026) | **existant** avec TetGen externe (gyroxide `298a7ce`, `gyroxide mesh`) ; isosurface stuffing à venir (gyroxide#5) |

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

## 5 · Description des outils — `--describe` (schéma `oxide-describe/1`)

**Statut : proposé (E58, 18 novembre 2026), à valider avant tout code dans les trois outils.**

guioxide ne code rien en dur d'une population : ce qu'une commande lit, écrit, accepte comme paramètres et demande
comme ressources, l'outil le dit lui-même. `medx --describe`, `surfx --describe` et `gyroxide --describe` impriment un
document JSON sur la sortie standard et sortent avec le code 0, sans rien lire ni écrire d'autre (ni poids, ni GPU,
ni jeu de données). Les profils de guioxide (P1, partie B) y relient les sorties aux étapes et à l'affichage.

### Règles

- **Version.** `schema` vaut `"oxide-describe/1"`. On ajoute des champs sans changer la version (un lecteur ignore ce qu'il
  ne connaît pas) ; on monte à `/2` quand un champ change de nom, de sens ou disparaît. `contract` donne la version
  du contrat de données (en-tête de ce document) que l'outil respecte.
- **Portée.** Seules les commandes qui ont un mode BIDS (`--bids`, `--participant`) sont décrites : ce sont celles que
  guioxide peut lancer et dont il retrouve les sorties. Les commandes de mise au point ou de mesure (`info`,
  `simulate`, `eval`, `device`, `compare`…) n'y figurent pas.
- **Clés en anglais**, comme `status.json` ; les textes d'aide gardent la langue de l'outil (celle de clap).
- **Motifs de chemin.** Un motif est relatif à la base de la commande : la racine BIDS pour une entrée `pipeline: null`,
  sinon `<étude>/derivatives/<pipeline>/`. Marques : `{sub}`, `{ses}`, `{acq}`, `{run}` (entités BIDS) ; `{x}` où `x` est
  l'identifiant d'un paramètre (sa valeur) ; `[…]` est un groupe facultatif, supprimé si une marque qu'il contient n'a pas de
  valeur (jeu sans session, pile sans `acq`). Le motif est la **seule** source du chemin : le code d'écriture de l'outil le
  construit à partir de la même table (voir « Vérification »).
- **Cardinalité.** `session` : un fichier par session ; `per_acquisition` : un fichier par acquisition brute de la
  session (un fichier `…[_acq-A][_run-N]_T2w.nii.gz`), `{acq}` et `{run}` prennent ceux de cette acquisition.
- **Développement d'une marque** (`expand`) : `{"hemi": {"param": "hemi", "all": ["L", "R"]}}` signifie que `{hemi}` prend la
  valeur du paramètre `hemi` s'il est donné, toutes les valeurs de `all` sinon (sans `all`, la valeur du paramètre, défaut compris).
- **Série** (`series`) : le nombre de fichiers n'est pas fixé (`{step}`, 7 chiffres) ; `index_output` désigne la sortie
  de nature `table` qui les énumère.

### Document

```
{ "schema": "oxide-describe/1",
  "contract": "1.0",
  "tool": { "name", "version", "commit"|null, "code_url" },
  "commands": [ Commande, … ] }
```

**Commande**

| Champ | Contenu | Origine |
|---|---|---|
| `id` | identifiant stable (`svr.recon`, `run`, `mesh`) | table |
| `argv` | mots à placer après le binaire (`["svr", "recon"]`) | clap (chemin de la sous-commande) |
| `summary` | résumé d'une ligne | clap (`about`, première phrase) |
| `stage` | étape générique nourrie : `acquisition`, `reconstruction`, `segmentation`, `surfaces`, `measures`, `simulation` ; le profil peut la réaffecter | table |
| `pipeline` | dossier des dérivées (`medx-svr`) ; vaut le `Name` du `dataset_description.json` | constante `PIPELINE_*` du code d'écriture |
| `generated_by` | `Name` écrit dans `GeneratedBy` (`medx svr recon`) : relie un dossier de dérivées à sa commande | table |
| `bids` | `{"mode": "required" \| "supported", "root": "--bids", "participant": "--participant", "session": "--session", "output_dir": "--output-dir"}` ; ces quatre arguments sont ceux de clap, dont guioxide fait la ligne de commande BIDS | clap (identifiants `bids`, `participant`, `session`, `output_dir`) + table pour `mode` ; `extra_required` : autres paramètres que clap exige seulement avec `--bids` (`label` pour `gyroxide run`), table |
| `inputs` | entrées (voir ci-dessous) | table |
| `outputs` | sorties (voir ci-dessous) | table |
| `parameters` | paramètres (voir ci-dessous) | clap, complété par la table |
| `presets` | jeux de valeurs nommés ; `default_preset` : celui que guioxide applique en mode BIDS si l'opérateur n'en choisit pas | table |
| `resources` | `{"gpu": "none" \| "optional" \| "required", "device_param": "device" \| null, "memory_gb": null \| {"typical": n}}` | table (`memory_gb` : seulement une valeur mesurée, sinon `null`) |
| `progress` | `{"pattern": …, "format": "status-json"}` (le `status.json` de la section 4) ou `null` tant que la commande n'en écrit pas | table |

**Entrée** : `{"id", "nature", "pipeline": null | "<dérivée>", "pattern", "cardinality", "count": "one" | "many", "required"}`.
`count: "many"` : tous les fichiers qui correspondent (`--input` restreint la liste).

**Sortie** : `{"id", "nature", "pattern", "required", "cardinality", "expand"?, "series"?, "table"?, "contents"?}`.
- `nature` : `volume`, `mask`, `labels`, `surface`, `field` (carte par sommet, `.shape.gii`), `table` (TSV), `mesh`
  (maillage volumique `.vtu`), `metadata` (JSON voisin), `run` (dossier d'une simulation). Elle décide de l'affichage ;
  le format du fichier reste celui de la section « Règles communes ».
- `required` : la commande écrit cette sortie à chaque exécution réussie dans le cas décrit (c'est ce que le test de
  l'outil vérifie). Une sortie facultative (`false`) ne conditionne pas l'état d'avancement.
- `table` (nature `labels`) : identifiant de la sortie `table` qui donne les étiquettes (`index`, `name`, `abbreviation`,
  `color`) : guioxide lit la table que l'outil écrit, il n'en code aucune.
- `contents` (nature `run`) : sorties imbriquées, motifs relatifs au dossier du run.

**Paramètre** : `{"id", "flag", "type", "default", "optional", "list"?, "choices"?, "min"?, "max"?, "unit"?, "level", "help"}`.
- `choices` : les valeurs de `ValueEnum` de clap ; quand l'outil n'a qu'un analyseur `FromStr` (`--hemi` de surfx), la table les donne.
- `type` : `number`, `integer`, `boolean` (drapeau), `string`, `path`, `enum` (alors `choices`). Pour clap : `f64` → `number`,
  entiers → `integer`, `bool` → `boolean`, `PathBuf` → `path`, `ValueEnum` → `enum`.
- `flag` : `--alpha`. `id` : l'identifiant clap (`min_stack_coverage`). `default` : la valeur par défaut de clap, `null` s'il n'y en a pas
  (la valeur dépend alors de la machine ou de l'entrée, ce que dit `help`).
- Ligne de commande d'une valeur : drapeau seul pour un booléen vrai ; sinon `flag valeur` ; pour une liste (`list: true`),
  `flag v1 v2 …`. Le paramètre n'est écrit que s'il diffère du défaut.
- `min`, `max`, `unit` : seulement s'ils sont connus de l'outil (clap n'en a pas) ; absents sinon, jamais devinés.
- `level` : `basic` (affiché d'emblée dans un formulaire) ou `advanced` (replié) ; tout est `advanced` sauf la liste `basic` de la table.
- Ne figurent **pas** parmi les paramètres : les quatre arguments BIDS ci-dessus, et les chemins du mode libre
  (`--input`, `--output`, `--mask`…) : la table les range en `free_io`.

### Exemples

Les exemples sont ceux que produirait l'outil, les listes de paramètres étant abrégées (`medx svr recon` a 44 arguments
clap ; le test de l'étape de code vérifie que le document réel les contient tous). Valeurs par défaut lues sur les binaires
compilés (medx, surfx, gyroxide 0.1.0).

**`medx svr recon`**

```json
{
  "id": "svr.recon",
  "argv": ["svr", "recon"],
  "summary": "Reconstruit un volume 3D à partir de stacks 2D (IRM fœtale) : estime le mouvement de chaque coupe, puis reconstruit",
  "stage": "reconstruction",
  "pipeline": "medx-svr",
  "generated_by": "medx svr recon",
  "bids": {"mode": "supported", "root": "--bids", "participant": "--participant", "session": "--session", "output_dir": "--output-dir"},
  "inputs": [
    {"id": "stacks", "nature": "volume", "pipeline": null,
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}][_acq-{acq}][_run-{run}]_T2w.nii.gz",
     "cardinality": "per_acquisition", "count": "many", "required": true},
    {"id": "masks", "nature": "mask", "pipeline": "medx-fetalbet",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}][_acq-{acq}][_run-{run}]_desc-brain_mask.nii.gz",
     "cardinality": "per_acquisition", "count": "many", "required": true}
  ],
  "outputs": [
    {"id": "volume", "nature": "volume", "required": true, "cardinality": "session",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_desc-svr_T2w.nii.gz"},
    {"id": "volume_sidecar", "nature": "metadata", "required": true, "cardinality": "session",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_desc-svr_T2w.json"},
    {"id": "motion", "nature": "table", "required": true, "cardinality": "session",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_desc-svr_motion.tsv"},
    {"id": "slicequality", "nature": "table", "required": true, "cardinality": "session",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_desc-svr_slicequality.tsv"}
  ],
  "parameters": [
    {"id": "motion", "flag": "--motion", "type": "enum", "choices": ["s2s", "s2v"], "default": "s2s", "optional": true, "level": "basic", "help": "Estimation du mouvement"},
    {"id": "device", "flag": "--device", "type": "enum", "choices": ["wgpu", "cpu", "cuda"], "default": null, "optional": true, "level": "advanced", "help": "Où se font les calculs. Par défaut : le CPU de la machine, choix fait par algorithme (le GPU est aujourd'hui plus lent)"},
    {"id": "resolution_mm", "flag": "--resolution-mm", "type": "number", "default": 0.8, "min": 0, "unit": "mm", "optional": true, "level": "basic", "help": "Côté des voxels du volume reconstruit, en mm"},
    {"id": "alpha", "flag": "--alpha", "type": "number", "default": 0.05, "min": 0, "optional": true, "level": "basic", "help": "Poids de la régularisation (lissage)"},
    {"id": "min_stack_coverage", "flag": "--min-stack-coverage", "type": "integer", "default": 2, "min": 1, "optional": true, "level": "advanced", "help": "Nombre minimal de stacks qui doivent atteindre un voxel pour qu'il soit reconstruit"},
    {"id": "s2s_sweeps", "flag": "--s2s-sweeps", "type": "integer", "default": 3, "min": 1, "optional": true, "level": "advanced", "help": "Avec `--motion s2s` : nombre de balayages de toutes les coupes"}
  ],
  "presets": [
    {"id": "s2v", "label": "Coupe-volume", "params": {"motion": "s2v"}}
  ],
  "default_preset": null,
  "resources": {"gpu": "optional", "device_param": "device", "memory_gb": null},
  "progress": null
}
```

(Le préréglage `s2v` est un exemple : son contenu réel se décide avec le propriétaire de l'outil à l'étape de code, et chaque
préréglage doit être accepté tel quel par clap, ce que le test vérifie.)

**`surfx run`**

```json
{
  "id": "run",
  "argv": ["run"],
  "summary": "Surfaces blanche et piale de chaque hémisphère, épaisseur corticale et rapport de contrat",
  "stage": "surfaces",
  "pipeline": "surfx-surfaces",
  "generated_by": "surfx run",
  "bids": {"mode": "supported", "root": "--bids", "participant": "--participant", "session": "--session", "output_dir": "--output-dir"},
  "inputs": [
    {"id": "segmentation", "nature": "labels", "pipeline": "medx-bounti",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_desc-bounti_dseg.nii.gz",
     "cardinality": "session", "count": "one", "required": true}
  ],
  "outputs": [
    {"id": "white", "nature": "surface", "required": true, "cardinality": "session",
     "expand": {"hemi": {"param": "hemi", "all": ["L", "R"]}},
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_white.surf.gii"},
    {"id": "pial", "nature": "surface", "required": true, "cardinality": "session",
     "expand": {"hemi": {"param": "hemi", "all": ["L", "R"]}},
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_pial.surf.gii"},
    {"id": "curv", "nature": "field", "required": true, "cardinality": "session",
     "expand": {"hemi": {"param": "hemi", "all": ["L", "R"]}},
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_curv.shape.gii"},
    {"id": "sulc", "nature": "field", "required": true, "cardinality": "session",
     "expand": {"hemi": {"param": "hemi", "all": ["L", "R"]}},
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_sulc.shape.gii"},
    {"id": "thickness", "nature": "field", "required": true, "cardinality": "session",
     "expand": {"hemi": {"param": "hemi", "all": ["L", "R"]}},
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_thickness.shape.gii"},
    {"id": "cortex", "nature": "field", "required": true, "cardinality": "session",
     "expand": {"hemi": {"param": "hemi", "all": ["L", "R"]}},
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_cortex.shape.gii"},
    {"id": "report", "nature": "metadata", "required": false, "cardinality": "session",
     "expand": {"hemi": {"param": "hemi", "all": ["L", "R"]}},
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_report.json"},
    {"id": "surfqc", "nature": "table", "required": true, "cardinality": "session",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_desc-surfqc.tsv"}
  ],
  "parameters": [
    {"id": "labels", "flag": "--labels", "type": "enum", "choices": ["bounti19", "multibounti43"], "default": "bounti19", "optional": true, "level": "basic", "help": "Table de labels : bounti19 ou multibounti43"},
    {"id": "hemi", "flag": "--hemi", "type": "enum", "choices": ["L", "R"], "default": null, "optional": true, "level": "basic", "help": "Hémisphère (L ou R) ; les deux par défaut"},
    {"id": "sigma", "flag": "--sigma", "type": "number", "default": 0.25, "min": 0, "unit": "mm", "optional": true, "level": "advanced", "help": "Écart-type (mm) du flou de la distance signée avant extraction"},
    {"id": "edge", "flag": "--edge", "type": "number", "default": 0.5, "min": 0, "unit": "mm", "optional": true, "level": "advanced", "help": "Longueur d'arête visée par le remaillage isotrope (mm) ; 0 : pas de remaillage"},
    {"id": "min_thickness", "flag": "--min-thickness", "type": "number", "default": 0.02, "min": 0, "unit": "mm", "optional": true, "level": "advanced", "help": "Distance minimale exigée entre surface blanche et piale (mm)"}
  ],
  "presets": [],
  "default_preset": null,
  "resources": {"gpu": "none", "device_param": null, "memory_gb": null},
  "progress": null
}
```

**`gyroxide run`**

```json
{
  "id": "run",
  "argv": ["run"],
  "summary": "Runs a simulation.",
  "stage": "simulation",
  "pipeline": "gyroxide",
  "generated_by": "gyroxide run",
  "bids": {"mode": "supported", "root": "--bids", "participant": "--participant", "session": "--session", "output_dir": "--output-dir", "extra_required": ["label"]},
  "inputs": [
    {"id": "mesh", "nature": "mesh", "pipeline": "gyroxide",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/sub-{sub}[_ses-{ses}]_hemi-{hemi}_desc-{mesh_desc}_tet.vtu",
     "expand": {"hemi": {"param": "hemi"}},
     "cardinality": "session", "count": "one", "required": true}
  ],
  "outputs": [
    {"id": "run", "nature": "run", "required": true, "cardinality": "session",
     "pattern": "sub-{sub}[/ses-{ses}]/anat/run-{label}/",
     "contents": [
       {"id": "config", "nature": "metadata", "required": true, "pattern": "config.toml"},
       {"id": "status", "nature": "metadata", "required": true, "pattern": "status.json"},
       {"id": "sequence", "nature": "table", "required": true, "pattern": "sequence.tsv"},
       {"id": "surfaces", "nature": "surface", "required": true,
        "expand": {"hemi": {"param": "hemi"}},
        "series": {"placeholder": "step", "index_output": "sequence"},
        "pattern": "sub-{sub}[_ses-{ses}]_hemi-{hemi}_desc-sim_step-{step}_pial.surf.gii"}
     ]}
  ],
  "parameters": [
    {"id": "label", "flag": "--label", "type": "string", "default": null, "optional": true, "level": "basic", "help": "Run label (letters and digits): run-<label>/."},
    {"id": "mesh_desc", "flag": "--mesh-desc", "type": "string", "default": "tetgen", "optional": true, "level": "advanced", "help": "desc- entity of the default session mesh in BIDS mode."},
    {"id": "preset", "flag": "--preset", "type": "string", "default": "tallinen2016", "optional": true, "level": "basic", "help": "Parameter preset."},
    {"id": "hemi", "flag": "--hemi", "type": "enum", "choices": ["L", "R"], "default": "L", "optional": true, "level": "basic", "help": "Hemisphere of the surface (GIfTI AnatomicalStructurePrimary, BIDS hemi- entity)."},
    {"id": "steps", "flag": "--steps", "type": "integer", "default": null, "min": 1, "optional": true, "level": "basic", "help": "Stop after this many steps (default: until t_end)."},
    {"id": "surface_every", "flag": "--surface-every", "type": "integer", "default": 0, "min": 0, "optional": true, "level": "advanced", "help": "Write the surface (GIfTI, image space in mm: inverse of the mesh normalisation) every N steps (0 = only the final surface)."},
    {"id": "growth_field", "flag": "--growth-field", "type": "string", "default": null, "optional": true, "level": "advanced", "help": "Growth mask gr read from a node field of the initial .vtu, e.g. `cortex`"},
    {"id": "no_midplane", "flag": "--no-midplane", "type": "boolean", "default": false, "optional": true, "level": "advanced", "help": "No midplane. Needed for a session hemisphere"},
    {"id": "backend", "flag": "--backend", "type": "enum", "choices": ["auto", "wgpu", "cuda", "ndarray"], "default": "auto", "optional": true, "level": "advanced", "help": "The fastest the machine offers…"}
  ],
  "presets": [
    {"id": "session-hemisphere", "label": "Hémisphère d'une session", "params": {"growth_field": "cortex", "no_midplane": true}}
  ],
  "default_preset": "session-hemisphere",
  "resources": {"gpu": "optional", "device_param": "backend", "memory_gb": null},
  "progress": {"pattern": "sub-{sub}[/ses-{ses}]/anat/run-{label}/status.json", "format": "status-json"}
}
```

Le document complet d'un outil enveloppe ces commandes : `{"schema": "oxide-describe/1", "contract": "1.0",
"tool": {"name": "gyroxide", "version": "0.1.0", "commit": "7ec21a2", "code_url": "https://github.com/rousseau/gyroxide"},
"commands": [ … ]}`.

### D'où vient chaque champ

- **clap** (`CommandFactory`, au moment de `--describe`, aucun texte copié) : `argv`, `summary`, `parameters` (`id`, `flag`,
  `type`, `default`, `choices`, `optional`, `list`, `help`), présence des arguments `bids`, `participant`, `session`, `output_dir`.
- **Table des sorties**, une par outil, dans le code qui écrit (`medoxide-svr::sortie`, `surfoxide-io::bids`,
  `gyroxide-io`) : `id`, `stage`, `pipeline`, `generated_by`, `bids.mode`, `inputs`, `outputs`, `presets`, `resources`,
  `progress`, `basic`, `free_io`, bornes et unités des paramètres. Les motifs y sont les chaînes à partir desquelles le code
  d'écriture construit ses chemins.
- **Constantes de compilation** : `tool.version` (`CARGO_PKG_VERSION`), `tool.commit` (`MEDX_COMMIT`, `SURFX_COMMIT`…, la variable que
  lit déjà `GeneratedBy`, `null` si absente), `tool.code_url`, `schema`, `contract`.

### Vérification

- À chaque `cargo test` : pour chaque sortie déclarée, le motif résolu sur un cas type (`sub-01`, `ses-01`, `acq-ax`, `run-1`)
  est comparé au chemin que construit la fonction d'écriture existante (`fichiers_bids`, `bids::derivative_dir`…) ; chaque
  préréglage passe l'analyse de clap ; chaque identifiant de paramètre de la table existe dans clap.
- Test marqué `#[ignore]`, sur le jeu fantôme de P0 ou un cas minimal : l'outil tourne en mode BIDS, et chaque sortie
  `required` du document est présente dans les dérivées.
- Côté guioxide : un test lit les exemples ci-dessus (copiés dans `scripts/jeu-test/` à l'étape de code) avec le même lecteur que
  guioxide, pour que le schéma ne dérive pas de ce texte.

### Ce que fait Boutiques autrement, et pourquoi on s'en écarte

D'après la spécification Boutiques (descripteur JSON : `command-line` en gabarit à clés `[CLÉ]`, `inputs` avec `value-key`
et `command-line-flag`, `output-files` avec `path-template`, `container-image`, `suggested-resources`, `groups`, `error-codes`,
`tests`) ; ces champs sont rappelés de mémoire, à revérifier avant publication.

- **Pas de gabarit de ligne de commande.** Boutiques décrit une ligne par un gabarit écrit à la main ; ici la ligne se
  déduit de clap (`flag valeur`), la même grammaire pour les trois outils, et l'outil ne peut pas décrire un drapeau qui
  n'existe pas (le document est généré, pas rédigé).
- **Sorties par motifs BIDS, pas par gabarits tirés des entrées.** Le `path-template` de Boutiques se construit à partir de
  la valeur d'un paramètre ; ici la sortie se retrouve dans une étude par son pipeline et ses entités (`sub`, `ses`, `acq`, `run`),
  y compris pour une session déjà traitée où aucune ligne de commande n'est rejouée.
- **Nature et étape.** Boutiques ignore ce qu'est un fichier (volume, étiquettes, surface, champ, run) et à quelle étape
  il appartient ; guioxide en a besoin pour afficher et déduire l'avancement.
- **Mode BIDS et mode libre.** Boutiques n'a qu'un mode ; ici les arguments BIDS et ceux du mode libre sont distingués.
- **Pas d'image de conteneur, de tests ni de codes d'erreur** : binaires natifs (Docker est hors du chemin des outils), tests dans
  les dépôts, code de sortie 0 ou non nul.
- **Ajouts propres à l'écosystème** : ressources GPU, fichier de progression, préréglages avec défaut, séries de sorties.
- Ce qu'on garde : identifiants, types, défauts, bornes, choix énumérés, groupe de ressources. Un export vers Boutiques reste
  possible plus tard ; il n'est pas livré.

## Évolutions prévues

- **Multi-contraste** : l'entité de suffixe (`T2w`, `T1w`, …) est conservée dans tous les
  noms ; guioxide liste les contrastes présents et les propose dans l'en-tête.
- **Diffusion** : données sous `dwi/` (`*_dwi.nii.gz`, `.bval`, `.bvec`) ; dérivées à
  définir (`desc-<…>_tensor`, cartes FA/MD) quand medoxide les produira.
