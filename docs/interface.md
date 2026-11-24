# Interface

Spécification des écrans, d'après la maquette (5 écrans : Ouverture + 4 onglets). Ce
document décrit **ce que chaque écran montre et d'où viennent les données** ; la maquette fait
foi pour la disposition.

## Gabarit commun aux onglets 1 à 4

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ guioxide  [jeu] / sub-X / ses-Y  [AG SA]   Contrastes: T2w +  │ Sortie │ Ouvrir │
├──────────────────────────────────────────────────────────────────────────────┤
│ Sessions › 1 Examen ✓masques ○contrôle › 2 Reconstruction ▶SVR 45 % … │ Réglages │ Lancer la suite │
├───────────────┬──────────────────────────────────────────┬───────────────────┤
│ Données de    │ Visualiseur                              │ Qualité /         │
│ l'étape       │ (barre d'outils + vues)                  │ paramètres /      │
│               │                                          │ « Valider → »     │
├───────────────┴──────────────────────────────────────────┴───────────────────┤
│ outil [version] · chemin des dérivées · tâches · mémoire GPU                 │
└──────────────────────────────────────────────────────────────────────────────┘
```

- **En-tête** : fil d'Ariane (jeu, sujet, session), âge gestationnel, sélecteur de contraste,
  pastille « Sortie : étude / test » (rappel permanent du répertoire de sortie ; clic → écran
  d'ouverture), bouton « Ouvrir un jeu BIDS ».
- **Barre d'étapes** (à la place des onglets et des anciennes fenêtres « Calculs » et « Suivi », voir
  [conception-visualiseur.md](conception-visualiseur.md)) : « Sessions », puis une étape par écran, avec l'état déduit
  du contrat et les pastilles de ses calculs (✓ fait, ○ à faire, ▶ avancement et temps restant, ✗ échec). Un clic sur
  une pastille ouvre son menu : lancer ce calcul, lancer jusqu'ici (ce qui manque avant), voir le journal. À droite,
  « Réglages » (résolution, mouvement, piles utilisées, hémisphère, pas) et « Lancer la suite » (tout ce qui manque,
  dans l'ordre), qui devient « Arrêter » pendant un calcul. Un écran dont l'étape n'est pas faite reste accessible et
  affiche ce qui manque.
- **Barre de pied** : provenance — outil et version (lus dans `dataset_description.json` de
  la dérivée), chemin des dérivées affichées, calcul en cours et temps restant ; « Journal » ouvre en tiroir, au bas de
  la fenêtre, la sortie de l'outil de chaque étape lancée.
- **Panneaux latéraux** repliables ; sous ~1100 px de large, ils passent en tiroirs.

## 0 · Ouverture

**État vide**

- Zone de dépôt : on y glisse un dossier ; un clic ouvre le sélecteur natif.
- Si le dossier n'est pas une racine BIDS, remonter jusqu'au parent qui contient
  `dataset_description.json` ; sinon, message d'erreur explicite.
- Liste des jeux récents (stockée dans les préférences), avec badge « étude » ou « test ».

**Jeu chargé**

- Colonne gauche : nom du jeu, chemin, version BIDS, nombre de sujets/sessions, dérivées
  trouvées, avertissements de validation ; filtres (contraste, « non terminé seulement »).
- Centre : tableau sujets × sessions — AG, contrastes disponibles, 4 pastilles d'état ; tri,
  recherche, sélection d'une ligne.
- Colonne droite : **répertoire de sortie**, deux choix exclusifs :
  1. *Étude — dérivées BIDS* (proposé par défaut) : `racine/derivatives/<pipeline>/` ;
  2. *Répertoire libre — tests* : chemin saisi ou parcouru ; options « partir des dérivées
     de l'étude » et « garder une arborescence BIDS ».
  Les états du tableau sont recalculés quand on change de sortie.
- Bouton « Ouvrir sub-X / ses-Y → » : onglet 1.

## 1 · Examen

- **Gauche** : piles `T2w` de la session (une ligne par fichier) : plan des coupes **dans le repère
  du scanner**, déduit de l'affine (axial / coronal / sagittal ; pas le plan anatomique du fœtus), nombre de coupes, résolution, score qualité
  (part de coupes conservées), badge si à revoir.
- **Centre** : la pile sélectionnée dans son plan natif ; curseur de coupe ; bande de
  vignettes des coupes, coupes signalées encadrées ; modes « pile », « mosaïque »,
  « comparer 2 piles » ; fenêtrage automatique.
- **Droite** : synthèse (piles utilisables, coupes exclues), histogramme du score par coupe
  avec seuil, masque `fetalbet` superposable, note de l'opérateur, « Valider → ».
- Source : piles brutes, masques `medx-fetalbet`, `*_slicequality.tsv`.

## 2 · Reconstruction

- **Sous-étapes** en tête : 2a masque, 2b reconstruction et mouvement, 2c segmentation. Chaque
  sous-étape a son état et ses paramètres (lus dans le `.json` voisin).
- **Gauche** : couches (volume, contour du masque, segmentation, pile source projetée) avec
  opacité ; table des étiquettes (couleur, nom, volume en cm³ calculé à l'affichage).
- **Centre** : vue triplanaire 2×2 (axial, coronal, sagittal + courbe de mouvement) ; curseur
  lié ; disposition 1×3 possible.
- **Courbe de mouvement** : par coupe, translation (mm) et rotation (°) tirées de
  `desc-svr_motion.tsv` ; coupes exclues surlignées ; sélection d'une pile.
- **Droite** : paramètres de la sous-étape, export NIfTI, « Valider → ».

## 3 · Surfaces

- **Gauche** : maillages trouvés (blanche, piale, médiane, par hémisphère) avec visibilité et
  couleur ; superposition 2D : contours des maillages, ou segmentation source, ou rien.
- **Centre** : trois coupes (contours = intersection maillage/plan) + grande vue 3D ;
  caméras prédéfinies (latérale, médiale, dorsale) ; curseur 2D ↔ 3D lié (clic sur un sommet
  → curseur monde ; déplacement du curseur → marqueur sur la surface) ; capture d'écran.
- **Droite** : coloration (courbure, profondeur sulcale, épaisseur, uni) + barre de couleur ;
  contrôle du maillage (sommets, faces, caractéristique d'Euler, auto-intersections, aire,
  indice de gyrification, distance à la segmentation), lu dans `desc-surfqc.tsv`.

## 4 · Simulation

- **Gauche** : runs de la session (`run-<label>`) avec état (`status.json`) ; paramètres du run
  sélectionné (lecture de `config.toml`).
- **Centre** : trois vues 3D synchronisées : état initial, pas courant, surface observée d'une
  session ultérieure du même sujet si elle existe ; frise temporelle (pas ↔ temps ↔ AG) avec
  lecture/pause ; barre de progression si le run est en cours.
- **Droite** : temps écoulé/restant, courbe de l'indice de gyrification simulé (et points
  observés), énergie, auto-contacts, distance simulé ↔ observé, fin du journal.
- Un run en cours est relu périodiquement (`status.json`, `sequence.tsv`) : les nouvelles
  surfaces apparaissent sans recharger.

## Interactions communes

Les mêmes dans toutes les vues de coupe ; le bouton `?` de chaque vue les rappelle.

| Action | Effet |
|---|---|
| Clic, glisser | place le curseur monde (toutes les vues suivent) |
| Molette, ↑ ↓, PageUp PageDown | coupe suivante / précédente |
| Pincer, Cmd + molette, `+` `−` | zoom |
| Maj + glisser, bouton du milieu | déplacer l'image |
| Option + glisser, bouton droit | contraste (horizontal : largeur, vertical : niveau) |
| Double-clic, `R` | recadrer la vue |
| `A` | contraste automatique |
| `F` | agrandir la vue survolée (vues triplanaires) ou revenir aux trois vues |
| `O` | montrer ou cacher la superposition (masque, segmentation) |
| `C` | superposition pleine ou en contour |
| Glisser en 3D | rotation ; Maj + glisser : translation |
| `1`…`4` | changer d'étape |
| `Ctrl/Cmd + O` | ouvrir un jeu BIDS |

Annotations aux coins de chaque vue : plan en haut à gauche ; valeur sous la souris (et étiquette) et numéro de coupe
en bas à gauche ; fenêtre (niveau N, largeur L) et zoom en bas à droite.

## Apparence

Coupes en **convention radiologique** (droite de la patiente à gauche de l'écran), lettres
d'orientation (R/L, A/P, S/I) sur les bords de chaque vue.


Thème sombre (lecture d'images) : fond `#111316`, panneaux `#181B20`, texte `#E6E8EB`, texte
secondaire `#9AA1AB`, accent rouille `#E07B39`, état « terminé » bleu `#7FB3E6`. Les états ne
reposent jamais sur la couleur seule (icône ou libellé). Polices : IBM Plex Sans et IBM Plex
Mono (licence OFL, embarquées dans le binaire).
