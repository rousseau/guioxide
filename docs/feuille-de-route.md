# Feuille de route

Chaque jalon se termine par une version utilisable et testée en CI sur Linux et macOS. On
commence par un **visualiseur** (guioxide lit les fichiers produits) ; le lancement des outils
vient en dernier.

## Jalon 0 — Contrat de données

- [ ] Relire et valider [donnees.md](donnees.md) avec les trois outils.
- [ ] Ouvrir les issues de [issues/](issues/) sur medoxide, surfoxide, gyroxide.
- [ ] Constituer le **jeu de test** (voir [developpement.md](developpement.md)).

*Fin* : le contrat est fusionné ; chaque ligne « à adapter » ou « proposé » a son issue.

## Jalon 1 — Squelette

- [x] Workspace Cargo (`guioxide`, `guioxide-bids`, `guioxide-data`, `guioxide-geom`,
      `guioxide-view`), `rust-toolchain.toml` commun.
- [x] Application `eframe` : en-tête, 5 onglets vides, barre de pied, thème et polices.
- [x] CI : `fmt`, `clippy -D warnings`, `test` sur `ubuntu-latest` et `macos-latest` ;
      compilation du binaire sur les deux.

*Fin* : `cargo run -p guioxide` ouvre la fenêtre sur Mac et sur Linux.

## Jalon 2 — Ouverture d'un jeu BIDS

- [x] `guioxide-bids` : racine (avec remontée depuis `sub-*`/`ses-*`), sujets, sessions,
      piles par contraste, `participants.tsv`, dérivées et versions.
- [x] État d'avancement par étape selon la table du contrat ; erreurs de lecture remontées.
- [x] Écran d'ouverture : dépôt de dossier, sélecteur natif, récents, tableau
      sujets × sessions, choix du répertoire de sortie.
- [x] Ouverture directe en argument : `guioxide /chemin/jeu`.

*Fin* : sur le jeu de test, le tableau affiche les bons états ; tests unitaires de
l'indexation sur des arborescences factices (dont cas invalides).

*État* : fait, sauf la vérification sur le jeu de test (pas encore constitué, jalon 0) ; essayé sur la
structure de `fetal-crl2025` (159 sujets, 193 sessions). Reportés : options « partir des dérivées de
l'étude » et « garder une arborescence BIDS » de la sortie libre (utiles au lancement des outils,
jalon 7) ; panneaux en tiroirs sous 1100 px.

## Jalon 3 — Visualiseur de coupes

- [x] `guioxide-geom` : affine (règle sform/qform), voxel ↔ monde, plans, échantillonnage.
- [x] `guioxide-data` : chargement NIfTI via `medoxide-core`, en arrière-plan, avec cache.
- [x] `guioxide-view` : vue de coupe (fenêtrage, zoom, déplacement), vue triplanaire avec
      curseur monde lié, superposition d'étiquettes (couleurs, opacité) et de contours.
- [ ] Tests de géométrie : un point monde tombe sur le même voxel que dans le SVR, sur des
      volumes obliques.

*Fin* : un volume de 0,5 mm iso se parcourt sans saccade ; masque et segmentation alignés.

*État* : géométrie testée contre nibabel sur l'affine d'une vraie pile oblique et sur une fonction
linéaire du monde (exacte en trilinéaire) ; lecture qform seul testée. La comparaison au SVR attend
que celui-ci lise ses piles par `affine_nifti` ([medoxide#1](https://github.com/rousseau/medoxide/issues/1)).
Rendu hors écran (`cargo run --release -p guioxide-view --example coupe`) sur une session de
`fetal-crl2025` : segmentation (0,5 mm) et masque (0,8 mm, qform seul) alignés ; composition d'une vue
en 4 ms. Fluidité dans la fenêtre : à confirmer à la main.

## Jalon 4 — Onglets Examen et Reconstruction

- [x] Examen : piles, vue native, vignettes, `slicequality.tsv`, masques.
- [x] Reconstruction : sous-étapes, triplanaire avec segmentation et `dseg.tsv`, courbe de
      mouvement (`motion.tsv`), paramètres (`.json`).

*Fin* : sur le jeu de test, toutes les sorties medoxide du contrat sont visibles.
Dépend des issues medoxide.

*État* : fait. Depuis medoxide `f2fbcb6` (21 oct. 2026), vérifié **sans conversion** sur les sorties de
`medx fetalbet / svr recon / bounti --bids` (sub-0104 de Marseille), lues par `medoxide_core::sorties`.
Avant cela : sorties réelles converties au format du contrat ; mouvement par coupe identique au calcul numpy/nibabel (`cargo run --release -p
guioxide-data --example svr`). Les sorties du contrat ne seront produites telles quelles qu'après les
issues [medoxide#2 à #5](https://github.com/rousseau/medoxide/issues). Reportés : mode « comparer 2
piles », couche « pile source projetée », export NIfTI, disposition 1 × 3 ; « Valider » et la note de
l'opérateur attendent un emplacement dans le contrat.

## Jalon 5 — Rendu 3D et onglet Surfaces

- [x] Lecture GIfTI (via `surfoxide-io` dès que disponible ; sinon module provisoire dans
      `guioxide-data`, à retirer ensuite).
- [x] Vue 3D wgpu : maillage, couleur par sommet, caméra orbitale, sélection de sommet.
- [x] Contours : intersection maillage/plan dans les coupes.
- [x] Onglet Surfaces complet, `surfqc.tsv`.

*Fin* : surfaces blanche et piale superposées aux coupes au voxel près. Dépend des issues
surfoxide.

*État* : fait sur les surfaces réelles de `surfx` (sub-0avbc / ses-02 de `fetal-crl2025`, segmentation
BOUNTI-19) : contours alignés sur le volume SVR dans les trois orientations, χ = 2 pour les quatre
surfaces. Lecture GIfTI par `surfoxide-io`, topologie et aire par `surfoxide-core` (le lecteur
provisoire de guioxide est retiré). Rendu 3D
vérifié sans fenêtre (`cargo run --release -p guioxide-view --example rendu3d`) et dans l'application.
Reporté : capture d'écran de la vue 3D ; `desc-surfqc.tsv` est lu s'il existe, sinon les mesures
(sommets, faces, χ, aire) sont calculées dans guioxide.

## Jalon 6 — Onglet Simulation

- [x] Lecture d'un run : `config.toml`, `sequence.tsv`, `status.json`, séquence GIfTI.
- [x] Frise temporelle, lecture, trois vues synchronisées, courbe d'indice de gyrification.
- [x] Relecture périodique d'un run en cours.

*Fin* : un run de gyroxide se rejoue ; les surfaces simulées sont dans le repère de l'image.
Dépend des issues gyroxide.

*État* : fait. Depuis gyroxide `298a7ce` (23 oct. 2026), vérifié **sans conversion** sur un run réel d'une
session (`gyroxide run --bids` depuis le `.vtu` de la piale de `surfx`) : surfaces en mm dans le repère de
l'image, suivi en direct d'un run en cours, runs lus par `gyroxide-io`. Avant cela : run d'atlas converti
au format du contrat. Le run se rejoue ; ses surfaces restent en coordonnées normalisées tant que
[gyroxide#2](https://github.com/rousseau/gyroxide/issues/2) n'est pas traitée (guioxide l'indique).
Issues [gyroxide#2 à #4](https://github.com/rousseau/gyroxide/issues) ouvertes. Reportés : distance
simulé ↔ observé, points d'indice de gyrification observés, fin du journal (pas de journal dans le
contrat).

## Jalon 7 — Lancer les outils

- [ ] Configuration des binaires (`medx`, `surfx`, `gyroxide`) : `PATH` ou chemins dans les
      préférences ; affichage de leur version.
- [ ] Exécution en sous-processus avec la ligne de commande générée depuis l'interface, sortie
      dans le répertoire choisi, journal, annulation.
- [ ] Progression : lecture de `status.json` (gyroxide) et des sorties standard.

*Fin* : une session va de l'examen à la simulation sans quitter guioxide.

## Validation par l'opérateur

- [x] Panneau « Validation » dans les quatre onglets : avis (validé, à revoir, rejeté), note, opérateur
      (mémorisé), version de l'outil lue dans `GeneratedBy` ; écriture atomique de
      `derivatives/guioxide/sub-X/ses-Y/sub-X_ses-Y_desc-qc.json` (voir [donnees.md](donnees.md)).
- [x] Colonne « Validation » dans le tableau des sessions, mise à jour à l'enregistrement.

## Plus tard

- Multi-contraste (T1w, …) : sélecteur actif, superposition de contrastes recalés.
- Diffusion : onglet dédié (cartes scalaires, tenseurs, tractographie) quand medoxide la produit.
- Revue de cohorte : enchaîner les sujets d'une étude pour le QC.
