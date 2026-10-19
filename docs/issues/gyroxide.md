# Issues à ouvrir sur gyroxide

Contexte commun à coller en tête de chaque issue :

> guioxide (https://github.com/rousseau/guioxide) visualise les sorties de la chaîne
> medoxide → surfoxide → gyroxide. Le contrat de données est dans `guioxide/docs/donnees.md`.

---

## 1. Surfaces simulées en GIfTI, dans le repère de l'image

Aujourd'hui `--vtk-every` écrit des surfaces en VTK legacy ASCII, dans le repère **normalisé**
de la simulation (`gyroxide-mesh::normalize`).

**Proposition**
- appliquer la transformée inverse de la normalisation avant écriture : sommets en RAS mm,
  repère de l'image source ;
- écrire en GIfTI via le lecteur/écrivain partagé de `surfoxide-core` (issue surfoxide n° 1) ;
- garder la sortie VTK legacy comme option de débogage (ParaView).

**Pourquoi** : l'onglet Simulation superpose les surfaces simulées aux coupes et aux surfaces
observées. Hors repère monde, rien ne s'aligne.

---

## 2. Un run = un dossier, avec index et état

**Proposition** : `gyroxide run --bids <racine> --participant X --session Y --label <run>`
écrit dans `<sortie>/derivatives/gyroxide/sub-X/ses-Y/anat/run-<label>/` :

- `config.toml` : paramètres complets (préréglage + surcharges), et chemin relatif de l'état
  initial (surface ou `.vtu` de surfoxide) ;
- `sequence.tsv` : `step time ga_weeks file gi energy n_contacts`, une ligne par sortie,
  ajoutée au fil du calcul ;
- `status.json` : `state` (`running` / `done` / `failed`), `step`, `total_steps`, `started`,
  `updated`, `message` ; réécrit de façon atomique (fichier temporaire puis renommage) ;
- `sub-X_ses-Y_hemi-L_desc-sim_step-<n>_pial.surf.gii` pour chaque sortie ;
- `derivatives/gyroxide/dataset_description.json` (`GeneratedBy`).

guioxide relit `status.json` et `sequence.tsv` pour suivre un run en cours.

---

## 3. Correspondance temps de simulation ↔ âge gestationnel

Pour afficher une frise en semaines d'aménorrhée, il faut une loi `t → AG`. À définir dans le
modèle (paramètre du préréglage ?) ; sinon guioxide affiche le temps du modèle seul.

---

## 4. Toolchain, CI, backends

Voir `guioxide/docs/developpement.md` :

- même version de Rust épinglée ;
- ajouter une CI (`ubuntu-latest` + `macos-latest`, tests sur le backend `ndarray`) ;
- tester la feature `cuda` sur la station (annoncée non testée) ;
- converger vers la même version de Burn que medoxide et surfoxide.
