# guioxide

Interface graphique de la suite *oxide* pour l'IRM fœtale : suivre un sujet de l'examen
acquis jusqu'à la simulation du plissement cortical.

| Étape | Outil | Ce que guioxide montre |
|---|---|---|
| 0 · Ouverture | — | jeu BIDS (choisi ou glissé-déposé), sujets/sessions, répertoire de sortie |
| 1 · Examen | [medoxide](https://github.com/rousseau/medoxide) | piles acquises, coupes, qualité par coupe |
| 2 · Reconstruction | medoxide (`medx fetalbet`, `medx svr`, segmentation) | masques, volume reconstruit, mouvement par coupe, segmentation |
| 3 · Surfaces | [surfoxide](https://github.com/rousseau/surfoxide) (`surfx`) | surfaces blanche/piale en triplanaire et en 3D, contrôle des maillages |
| 4 · Simulation | [gyroxide](https://github.com/rousseau/gyroxide) | séquence de surfaces simulées, comparaison à l'observé |

guioxide **lit** ce que les outils ont écrit, dans un jeu BIDS et ses dérivées. Plus tard, il
**lancera** les outils comme des programmes séparés ; il ne les lie pas comme bibliothèques
(voir [architecture](docs/architecture.md)).

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
