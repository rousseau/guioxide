# guioxide

Interface graphique de la suite *oxide* pour l'IRM fœtale : suivre un sujet de l'examen
acquis jusqu'à la simulation du plissement cortical.

| Étape | Outil | Ce que guioxide montre |
|---|---|---|
| 0 · Ouverture | — | jeu BIDS (choisi ou glissé-déposé), sujets/sessions, répertoire de sortie ; ou nouvelle séance à partir de piles NIfTI |
| 1 · Examen | [medoxide](https://github.com/rousseau/medoxide) | piles acquises, coupes, qualité par coupe |
| 2 · Reconstruction | medoxide (`medx fetalbet`, `medx svr`, segmentation) | masques, volume reconstruit, mouvement par coupe, segmentation |
| 3 · Surfaces | [surfoxide](https://github.com/rousseau/surfoxide) (`surfx`) | surfaces blanche/piale en triplanaire et en 3D, contrôle des maillages |
| 4 · Simulation | [gyroxide](https://github.com/rousseau/gyroxide) | séquence de surfaces simulées, comparaison à l'observé |

guioxide **lit** ce que les outils ont écrit, dans un jeu BIDS et ses dérivées. Il **lance** aussi
les outils comme des programmes séparés ; il ne les lie pas comme bibliothèques
(voir [architecture](docs/architecture.md)).

### Nouvelle séance à partir de piles NIfTI, calculs

Glisser des fichiers `.nii` / `.nii.gz` dans la fenêtre (ou bouton « Nouvelle séance… ») : guioxide crée un jeu BIDS
(`sub-X/ses-Y/anat/sub-X_ses-Y[_acq-…][_run-N]_T2w.nii.gz` ; participant, session, `acq` et `run` repris du nom
d'origine ; chemin d'origine dans le `.json` de chaque pile ; âge gestationnel dans `sub-X_sessions.tsv`) et l'ouvre
sur l'onglet Examen pour vérifier les piles. Rien n'est lancé d'office.

Bouton « Calculs » (toute session choisie, importée ou non) : chaque étape se lance seule, ou une sélection à la
suite — masques (`medx fetalbet`), contrôle des piles (`medx svr qc`), reconstruction (`medx svr recon` : résolution,
s2s ou s2v, régularisation, CPU ou GPU, piles utilisées), segmentation (`medx bounti`), surfaces (`surfx run`),
maillage et simulation (`gyroxide mesh`, `gyroxide run`). Durée estimée d'après les exécutions précédentes sur la
machine ; fenêtre « Suivi » : avancement, fin du journal, annulation ; journaux dans `derivatives/guioxide/journaux/`.
Les onglets se mettent à jour à la fin de chaque étape.

Vues de coupe : pincer ou Cmd + molette pour zoomer, Maj + glisser pour déplacer, Option + glisser pour le contraste,
molette pour changer de coupe ; boutons « Recadrer » et « Contraste auto » au survol.

Binaires cherchés dans cet ordre : `GUIOXIDE_MEDX`, `GUIOXIDE_SURFX`, `GUIOXIDE_GYROXIDE` ; le `PATH` complété de
`~/.cargo/bin`, `~/.local/bin`, `/opt/homebrew/bin` ; `target/release/` des clones (`$OXIDE_RACINE`, défaut
`~/Exp/oxide`). TetGen : `GYROXIDE_TETGEN` ou le même `PATH`.

Plateformes visées : macOS (Apple Silicon) et Linux, au minimum.

## Documentation

| Document | Contenu |
|---|---|
| [docs/architecture.md](docs/architecture.md) | crates, dépendances, rendu, choix structurants |
| [docs/donnees.md](docs/donnees.md) | **contrat de données** : fichiers attendus de chaque outil, chemins BIDS, formats, repères |
| [docs/interface.md](docs/interface.md) | spécification des écrans (d'après la maquette) |
| [docs/feuille-de-route.md](docs/feuille-de-route.md) | jalons et critères de fin |
| [docs/developpement.md](docs/developpement.md) | conventions communes aux quatre dépôts (toolchain, plateformes, CI, données de test) |
| [docs/issues/](docs/issues/) | demandes à ouvrir sur medoxide, surfoxide et gyroxide |

## Démarrage

```bash
cargo run --release -p guioxide
cargo run --release -p guioxide -- /chemin/vers/jeu_bids      # ouvre directement un jeu
```

### Application macOS (Dock)

```bash
bash scripts/macos/installer.sh    # construit depuis le commit courant et installe dans /Applications
```

L'installation remplace `/Applications/guioxide.app` (ou `~/Applications` si `/Applications` n'est pas accessible en
écriture) ; l'icône gardée dans le Dock lance la nouvelle version. Si guioxide tourne, le script attend : le quitter puis
relancer avec `--sans-construire`. Journal : `~/Library/Logs/guioxide-installations.log`.

Pour une image disque à copier sur un autre Mac : `bash scripts/macos/dmg.sh` (→ `target/dmg/guioxide-<version>.dmg`).
Signature ad hoc : sur un autre Mac, clic droit → Ouvrir au premier lancement.
