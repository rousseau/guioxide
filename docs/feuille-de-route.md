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

- [ ] `guioxide-geom` : affine (règle sform/qform), voxel ↔ monde, plans, échantillonnage.
- [ ] `guioxide-data` : chargement NIfTI via `medoxide-core`, en arrière-plan, avec cache.
- [ ] `guioxide-view` : vue de coupe (fenêtrage, zoom, déplacement), vue triplanaire avec
      curseur monde lié, superposition d'étiquettes (couleurs, opacité) et de contours.
- [ ] Tests de géométrie : un point monde tombe sur le même voxel que dans le SVR, sur des
      volumes obliques.

*Fin* : un volume de 0,5 mm iso se parcourt sans saccade ; masque et segmentation alignés.

## Jalon 4 — Onglets Examen et Reconstruction

- [ ] Examen : piles, vue native, vignettes, `slicequality.tsv`, masques.
- [ ] Reconstruction : sous-étapes, triplanaire avec segmentation et `dseg.tsv`, courbe de
      mouvement (`motion.tsv`), paramètres (`.json`).

*Fin* : sur le jeu de test, toutes les sorties medoxide du contrat sont visibles.
Dépend des issues medoxide.

## Jalon 5 — Rendu 3D et onglet Surfaces

- [ ] Lecture GIfTI (via `surfoxide-core` dès que disponible ; sinon module provisoire dans
      `guioxide-data`, à retirer ensuite).
- [ ] Vue 3D wgpu : maillage, couleur par sommet, caméra orbitale, sélection de sommet.
- [ ] Contours : intersection maillage/plan dans les coupes.
- [ ] Onglet Surfaces complet, `surfqc.tsv`.

*Fin* : surfaces blanche et piale superposées aux coupes au voxel près. Dépend des issues
surfoxide.

## Jalon 6 — Onglet Simulation

- [ ] Lecture d'un run : `config.toml`, `sequence.tsv`, `status.json`, séquence GIfTI.
- [ ] Frise temporelle, lecture, trois vues synchronisées, courbe d'indice de gyrification.
- [ ] Relecture périodique d'un run en cours.

*Fin* : un run de gyroxide se rejoue ; les surfaces simulées sont dans le repère de l'image.
Dépend des issues gyroxide.

## Jalon 7 — Lancer les outils

- [ ] Configuration des binaires (`medx`, `surfx`, `gyroxide`) : `PATH` ou chemins dans les
      préférences ; affichage de leur version.
- [ ] Exécution en sous-processus avec la ligne de commande générée depuis l'interface, sortie
      dans le répertoire choisi, journal, annulation.
- [ ] Progression : lecture de `status.json` (gyroxide) et des sorties standard.

*Fin* : une session va de l'examen à la simulation sans quitter guioxide.

## Plus tard

- Multi-contraste (T1w, …) : sélecteur actif, superposition de contrastes recalés.
- Diffusion : onglet dédié (cartes scalaires, tenseurs, tractographie) quand medoxide la produit.
- Revue de cohorte : enchaîner les sujets d'une étude pour le QC.
