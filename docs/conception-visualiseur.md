# Visualiseur : ce qu'on reprend des autres logiciels

Note du 24 novembre 2026, après un premier essai sur une séance réelle : l'interface est plus moderne mais pas
intuitive, et la fenêtre flottante de suivi des calculs gêne. But : le minimum d'un visualiseur (zoom, déplacement,
recentrage, contraste, superposition, contours), dans une interface épurée, sans les panneaux chargés des logiciels
de recherche.

## Ce que font les autres

| Logiciel | Ce qui marche | Ce qu'on évite |
|---|---|---|
| **ITK-SNAP** | trois coupes et la 3D en 2 × 2, chaque vue s'agrandit d'un clic et revient ; curseur partagé ; une seule opacité pour toute la segmentation ; contraste automatique d'un clic | menus profonds, fenêtres d'outils séparées |
| **FSLeyes** | liste des couches (œil, ordre) et une barre d'affichage pour la couche choisie : opacité, plage, palette ; étiquettes en contour seulement ; panneau de position (coordonnées et valeur de chaque couche) | trop de panneaux ouverts par défaut, réglages en double |
| **Freeview** | dispositions 1 × 1, 2 × 2, 1 + 3 ; volumes d'étiquettes « en contour » ; PageUp/PageDown et flèches pour changer de coupe ; remise à zéro de la vue au clavier | barre d'outils dense, icônes peu lisibles |
| **OsiriX / Horos** | bande de vignettes des séries à gauche ; annotations aux quatre coins (fenêtre, zoom, numéro de coupe, position) au lieu de panneaux ; préréglages de fenêtre ; images marquées « clés » | outils de mesure et de traitement tous visibles |
| **Consoles Siemens, Philips** (syngo, ExamCard) | la chaîne d'examen est une suite d'étapes visibles en permanence, chacune avec son état et son avancement en place ; les reconstructions arrivent d'elles-mêmes dans l'examen ; paramètres de l'étape à côté de l'étape | fenêtres modales, vocabulaire constructeur |

## Ce qu'on retient

1. **Un seul modèle d'interaction**, le même dans toutes les vues (piles, masques, volume, segmentation, surfaces) :

   | Geste | Effet |
   |---|---|
   | clic, glisser | placer le curseur (toutes les vues suivent) |
   | molette, ↑ ↓, PageUp PageDown | coupe suivante ou précédente |
   | pincer, Cmd + molette, `+` `−` | zoom autour de la souris |
   | Maj + glisser (ou bouton du milieu) | déplacer l'image |
   | Option + glisser (ou bouton droit) | contraste : horizontal la largeur, vertical le niveau |
   | double-clic, `R` | recadrer |
   | `A` | contraste automatique |
   | `F` | agrandir la vue survolée, ou revenir aux trois vues |
   | `O` | montrer ou cacher la superposition (masque, segmentation) |
   | `C` | superposition pleine ou en contour |

   Les gestes ne sont plus écrits en permanence dans la vue : un bouton `?` les affiche sur la vue, d'un clic.
2. **Annotations aux coins** (OsiriX) plutôt que panneaux : nom du plan en haut à gauche ; position de la coupe en
   bas à gauche ; fenêtre (niveau, largeur) et zoom en bas à droite ; valeur sous la souris en haut à droite.
3. **Superposition** (ITK-SNAP, FSLeyes, Freeview) : une opacité, un mode plein ou contour, un raccourci pour la
   cacher. Les étiquettes en contour laissent voir l'image dessous, ce qui sert à juger une segmentation.
4. **Agrandir une vue** (ITK-SNAP) : bouton au survol de chaque vue, ou `F`.
5. **La chaîne de calcul est une barre d'étapes** (consoles de scanner), sous l'en-tête, à la place des onglets et des
   deux fenêtres flottantes « Calculs » et « Suivi » :
   - chaque étape (1 Examen, 2 Reconstruction, 3 Surfaces, 4 Simulation) est un onglet qui montre ses calculs, chacun
     avec son état (✓ fait, ○ à faire, avancement en cours, ✗ échec) ; un clic sur un calcul propose de le lancer, avec
     la commande et la durée estimée ;
   - à droite, « Lancer la suite » enchaîne les calculs qui manquent, dans l'ordre ; pendant un calcul, ce bouton devient
     « Arrêter » ; « Réglages » ouvre, sous le bouton, résolution, mouvement, piles utilisées, hémisphère ;
   - l'avancement et le temps restant s'affichent dans la barre d'étapes et dans la barre de pied ; le journal de
     l'outil s'ouvre en tiroir au bas de la fenêtre (« Journal »), sans masquer les images.

## Ce qu'on ne fait pas

- Pas d'outils d'édition (pinceau, régions) : guioxide montre et lance, les outils calculent.
- Pas de palettes multiples pour les images : niveaux de gris ; couleurs réservées aux étiquettes et aux champs de
  surface.
- Pas de fenêtres flottantes pour le travail courant ; les seules restantes sont la création d'une séance et les
  menus.
