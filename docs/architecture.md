# Architecture

## Principes

1. **guioxide lit des fichiers, il ne lie pas les outils.** medoxide, surfoxide et gyroxide
   dépendent chacun de Burn, dans trois versions différentes (`0.22.0-pre.4`, `0.22.0`,
   `0.21.0`). Lier l'un d'eux dans l'interface imposerait sa version de Burn (et de wgpu) aux
   autres. Le seul couplage entre les dépôts est donc le **contrat de données**
   ([donnees.md](donnees.md)) : chemins, formats, repères.
2. **Exception : les crates « cœur » sans Burn.** Ils sont réutilisés pour que guioxide
   interprète les fichiers exactement comme les outils qui les ont écrits :
   - `medoxide-core` : lecture NIfTI (en-tête, affine, voxels) ;
   - `surfoxide-core` : maillage triangulaire, topologie, mesures ;
   - `surfoxide-io` : lecture/écriture GIfTI (et `.vtu`), sans Burn (voir
     [issues/surfoxide.md](issues/surfoxide.md)).
   Ils sont tirés en dépendance git, épinglés par `rev`.
3. **Exécution par sous-processus** (jalon 7). guioxide appelle `medx`, `surfx` et `gyroxide`
   comme des programmes, suit leur sortie et relit les fichiers produits. Un plantage GPU
   d'un outil ne ferme pas l'interface ; chaque outil garde son propre cycle de versions.
4. **guioxide ne fait que l'interface.** Toute lecture, écriture ou traitement de données (formats,
   mesures, mouvement, indices) vient des crates des outils ; guioxide les adapte à l'affichage. Ce qui
   manque se demande par une issue sur l'outil, plutôt que d'être réécrit dans guioxide. Restent dans
   guioxide : la navigation (indexation BIDS, état d'avancement), l'échantillonnage pour l'affichage, le
   rendu. Depuis le 23 oct. 2026, guioxide ne lit plus aucun fichier d'outil lui-même : NIfTI et sorties
   du SVR par `medoxide-core`, surfaces et contrôle des maillages par `surfoxide-io`, runs par
   `gyroxide-io`.
   Seule écriture de guioxide : ses propres annotations de validation
   (`derivatives/guioxide/…/desc-qc.json`, voir [donnees.md](donnees.md)), jamais dans les dérivées
   des outils.
5. **Toute la logique testable hors de l'interface.** L'indexation BIDS, la lecture des
   données et la géométrie sont dans des crates sans dépendance graphique, testés en CI.

## Crates du workspace

```
crates/
  guioxide-bids/     indexation d'un jeu BIDS et de ses dérivées ; état d'avancement par sujet/session
  guioxide-data/     chargement : volumes (via medoxide-core), maillages (via surfoxide-io),
                     TSV (qualité, poses, étiquettes), séquences de simulation ; cache
  guioxide-geom/     repères : voxel ↔ monde (RAS, mm), plans de coupe, intersection maillage/plan
  guioxide-view/     widgets de visualisation : vue de coupe, vue triplanaire, vue 3D, courbes
  guioxide/          binaire : application eframe, onglets, état global, préférences
```

Dépendances autorisées, de bas en haut : `geom` ← `data` ← `bids` ← `view` ← `guioxide`.
`guioxide-bids`, `-data` et `-geom` ne dépendent ni d'egui ni de wgpu.

## Pile graphique

| Besoin | Choix | Remarque |
|---|---|---|
| Fenêtre, widgets, onglets | `eframe` / `egui` | immédiat, natif Mac et Linux, glisser-déposer de dossiers géré par la fenêtre |
| Courbes (mouvement, qualité, indice de gyrification) | `egui_plot` | |
| Coupes 2D | textures egui (upload d'une coupe 2D déjà échantillonnée sur CPU) | simple ; passer au GPU seulement si la fluidité l'exige |
| Rendu 3D des maillages | callback wgpu dans egui (`egui_wgpu::CallbackTrait`) | une passe : maillage, normales, couleur par sommet, caméra orbitale |
| Sélecteur de répertoire | `rfd` | boîte de dialogue native |

Backend wgpu : Metal sur Mac, Vulkan sur Linux (station RTX 2080 Ti, pilote NVIDIA).

### Décision : egui natif plutôt que Tauri (24 nov. 2026)

Les autres applications Rust du laboratoire (nonos, rusty_music, spinloop) sont en Tauri. guioxide
reste en `eframe` / `egui`. Le choix initial n'avait pas été comparé à Tauri ; cette note le fait
après coup.

**Raisons du choix d'egui**

1. **Les données restent en Rust, sans copie.** Volumes NIfTI de plusieurs centaines de Mo, maillages
   de centaines de milliers de triangles, lus par `medoxide-core` et `surfoxide-io`. Avec egui, une coupe
   échantillonnée devient une texture et un maillage un tampon wgpu, dans le même processus. Avec Tauri,
   il faudrait les faire passer vers la webview (IPC, ou protocole personnalisé en binaire), puis refaire
   le rendu en JS.
2. **Rendu 3D en wgpu natif, sur les trois architectures.** La vue 3D est un callback wgpu
   (`egui_wgpu::CallbackTrait`) : Metal sur Mac, Vulkan sur Linux, la même pile graphique que Burn et
   gyroxide. Dans une webview, WebGL ou WebGPU dépend du moteur (WKWebView sur Mac, WebKitGTK sous
   Linux), où WebGPU est encore limité ; or guioxide doit tourner sous Linux aarch64 et x86_64 avec
   pilote NVIDIA.
3. **Une seule langue, une seule chaîne.** Ni npm ni front JS : la même toolchain épinglée et la même CI
   que les trois outils ([developpement.md](developpement.md)). La logique (`guioxide-bids`, `-data`,
   `-geom`) se teste en CI comme le reste.
4. **Une visionneuse plutôt qu'une application de formulaires.** Coupes liées par un curseur en
   coordonnées monde, courbes, rafraîchissement continu à chaque déplacement : le mode immédiat d'egui
   s'y prête. Tauri est plus fort pour une interface riche en mise en page et en texte.

**Alternative écartée : Tauri + [niivue](https://github.com/niivue/niivue)**

C'est la seule alternative sérieuse. niivue (WebGL2) affiche déjà volumes NIfTI, superposition
d'étiquettes, maillages et lecture GIfTI ; une bonne part de `guioxide-view` serait déjà faite, et
l'interface aurait le soin et la pile des autres applications. Elle est écartée pour l'instant parce
qu'elle :

- ferait relire les fichiers par niivue (JS) plutôt que par les crates des outils, contre le principe 4
  (guioxide ne lit aucun fichier d'outil lui-même), ou imposerait de transférer les données lues en Rust ;
- ajouterait une seconde pile (TypeScript, npm, tests côté web) aux quatre dépôts Rust ;
- dépendrait du WebGL de WebKitGTK sous Linux, non vérifié sur les stations NVIDIA.

**Quand rouvrir la question**

- si les vues propres à guioxide (coupe, triplanaire, 3D) coûtent nettement plus à maintenir que ce
  qu'apporterait niivue ;
- si l'interface doit s'ouvrir hors des trois machines (navigateur, collègues sans compilation) ;
- si egui bloque sur un besoin d'interface (texte riche, accessibilité, mise en page complexe).

Avant de changer, un essai borné : un sujet du jeu de test affiché dans niivue sous Tauri, sur le Mac et
la station x86_64, avec les données lues par les crates des outils puis transmises à la webview, temps
de chargement et fluidité comparés à guioxide.

## Modèle d'état

```
Application
├── jeu : Option<JeuBids>            racine, sujets, sessions, dérivées trouvées
├── sortie : Sortie                  Etude (racine/derivatives) | Libre(chemin)
├── selection : Option<(Sujet, Session)>
├── onglet : Onglet                  Ouverture | Examen | Reconstruction | Surfaces | Simulation
├── curseur : PointMonde             partagé par toutes les vues d'un onglet (RAS, mm)
└── cache : Cache                    volumes et maillages chargés, évincés par taille
```

- Le **curseur est en coordonnées monde**, jamais en indices de voxel : c'est ce qui permet de
  lier coupes, volumes de résolutions différentes et maillages.
- Les chargements longs (volume, maillage) se font sur un fil d'arrière-plan ; l'interface
  affiche un état « chargement » et reste réactive.
- L'**état d'avancement** d'une étape est déduit de la présence et de la validité des
  fichiers du contrat, dans le répertoire de sortie choisi. guioxide n'a pas de base de
  données propre.

## Géométrie : règle unique

- Affine voxel → monde : `sform` si `sform_code > 0`, sinon `qform` si `qform_code > 0`,
  sinon diagonale des `pixdim` (avec avertissement). **La même règle doit être appliquée par le
  SVR** : elle doit vivre dans `medoxide-core` (voir [issues/medoxide.md](issues/medoxide.md)).
- Repère monde : RAS, mm (convention NIfTI). Les maillages sont dans ce même repère
  (convention de surfoxide). Aucun outil n'écrit de fichier dans un repère normalisé ou
  propre à l'outil.
- Vue de coupe : on échantillonne le volume sur un plan défini dans le monde (trilinéaire pour
  l'image, plus proche voisin pour les étiquettes). Les vues axiale, coronale et sagittale
  sont des plans alignés sur les axes monde ; la vue « pile » d'une acquisition est le plan de
  la coupe native.

## Ce qui n'est pas dans guioxide

- Aucun calcul de traitement (masque, SVR, segmentation, surfaces, simulation).
- Aucun calcul de QC : les indicateurs sont produits par les outils et seulement lus.
- Pas de comparaison entre sorties « étude » et « test » : le répertoire de sortie choisi est
  le seul lu.
