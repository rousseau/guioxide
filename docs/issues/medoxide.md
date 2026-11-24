# Issues à ouvrir sur medoxide

> **Toutes faites** (21 oct. 2026, medoxide `8ab49a6` … `f2fbcb6`) : mode `--bids` de fetalbet, du SVR et de
> bounti, `motion.tsv`, `slicequality.tsv` (avec `excluded`, `reason`), affine des piles, lecteurs
> `medoxide_core::sorties`. Vérifié de bout en bout sur un sujet du jeu de développement (jeu `dev-guioxide/<sujet>-medx`).

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

> **Ouverte** : [medoxide#2](https://github.com/rousseau/medoxide/issues/2).

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

> **Ouverte** : [medoxide#3](https://github.com/rousseau/medoxide/issues/3).

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

> **Ouverte** : [medoxide#4](https://github.com/rousseau/medoxide/issues/4).

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

> **Ouverte** : [medoxide#5](https://github.com/rousseau/medoxide/issues/5).

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

---

Demandes 7 à 10 : premier essai de guioxide sur une séance réelle (5 piles HASTE de 19 coupes, 512 × 512,
0,74 × 0,74 × 4,6 mm), le 24 novembre 2026 sur le Mac, `medx` du 5 novembre. **À ouvrir.**

## 7. Orientation du volume reconstruit avant la segmentation

Constat : `medx svr recon` garde l'orientation des piles (repère de l'appareil) ; le cerveau fœtal y est tourné de
façon quelconque (ici d'environ 90° : la vue coronale du volume montre une coupe sagittale du cerveau). La chaîne
officielle de BOUNTI-19 rééchantillonne dans une grille LAS **sans réorienter** : elle suppose un volume déjà dans
l'orientation de l'atlas, comme le rend SVRTK. Résultat : segmentation très mauvaise sur un volume pourtant correct
(liquide externe gauche + droit 116 cm³ sur 326, hémisphères mêlés).

Demande, au choix :
- l'étape 5 de `docs/ROADMAP.md` (recalage rigide du volume reconstruit sur un atlas, réorientation) dans
  `medx svr recon`, ou en commande séparée que guioxide lancera avant `medx bounti` ;
- ou `medx multibounti --bids` (Multi-BOUNTI réoriente lui-même, par le réseau gauche/droite et le recalage affine
  sur son gabarit), avec un chemin de sortie du contrat que `surfx run --labels multibounti43` lise ; il demande une
  reconstruction à 0,5 mm, que guioxide sait désormais demander (`--resolution-mm`).

## 8. Avancement et parallélisme de la SVR coupe-coupe

Constat : `medx svr recon --motion s2s --device cpu` sur 95 coupes : 781 s, un seul cœur occupé la plupart du temps
(mise à jour `gauss-seidel`, `s2s_block_parallel_gradient: false`), et rien dans la sortie standard entre la ligne
de départ et la fin : l'utilisateur ne sait pas où en est le calcul. guioxide affiche une estimation tirée des
exécutions précédentes, faute de mieux.

Demande :
- écrire l'avancement : une ligne par passe ou par pile (`passe 2/3, pile 4/5`), ou mieux `status.json` au format de
  gyroxide (`docs/donnees.md`), que guioxide lira pour une vraie barre d'avancement (P2 de la feuille de route) ;
- paralléliser ce qui peut l'être sur CPU (recalages de coupes indépendantes, mise à jour par blocs) ou dire dans
  `--help` quels réglages le permettent et à quel prix.

## 9. Exclure des coupes à la main

Constat : l'opérateur voit dans l'examen des coupes à écarter (coupes sombres, coupe 14 de la première pile ici,
utilisée par le SVR) ; `--input` (avec `--bids`) ne permet d'écarter que des piles entières.

Demande : une option `--exclude <fichier.tsv>` (colonnes `stack slice`, ou le `desc-qc` de guioxide) lue par
`svr qc` et `svr recon`, les coupes listées étant marquées `excluded = 1`, motif `operateur`, dans
`desc-svr_slicequality.tsv`. guioxide écrira ce fichier dans `derivatives/guioxide/` depuis la mosaïque.

## 10. Coupes « hors masque » au bord d'une pile

Constat : pile coronale propre, 5 coupes sur 19 exclues `hors_masque` (0, 1, 16, 17, 18). Si le masque de fetalbet
est vide sur des coupes qui montrent encore le cerveau, la reconstruction perd des données. À vérifier sur cette
séance : les coupes 16 à 18 contiennent-elles du cerveau ? Si oui, Fetal-BET manque les coupes tangentes au cerveau
(seuil, plus grande composante), et un masque dilaté d'une coupe dans l'axe z, ou un seuil de couverture plus bas,
serait à évaluer.
