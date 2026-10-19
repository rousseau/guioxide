# Développement : conventions communes

guioxide relie quatre dépôts développés sur deux machines : medoxide sur Mac (Apple Silicon,
Metal), surfoxide et gyroxide sur la station Linux RTX 2080 Ti (CUDA). Le but n'est pas
d'avoir une seule machine, mais que **chaque outil compile, se teste et produise les mêmes
fichiers sur les deux**.

## État actuel (octobre 2026)

| | medoxide | surfoxide | gyroxide |
|---|---|---|---|
| Machine de dev | Mac (Metal) | station Linux RTX 2080 Ti | station Linux RTX 2080 Ti |
| Toolchain | `stable`, non épinglée → 1.99.1 (branche prête, en attente) | **1.99.1** ([PR #1](https://github.com/rousseau/surfoxide/pull/1)) | **1.99.1** ([PR #1](https://github.com/rousseau/gyroxide/pull/1)) |
| Burn | `0.22.0-pre.4`, `wgpu` | `0.22.0`, features `wgpu` / `cuda` | `0.21.0`, `wgpu` par défaut, `cuda` prévu (non testé) |
| CI | documentation seulement | fmt, clippy, test, selftest ; ubuntu + macos | fmt, clippy, test ; ubuntu + macos |

## Ce qu'il faut harmoniser

### 1. Toolchain : une version épinglée, la même partout

Même `rust-toolchain.toml` dans les quatre dépôts, avec une version exacte (la dernière
stable au moment de la montée ; ≥ 1.95, imposée par surfoxide) et les composants `rustfmt` et
`clippy`. On la monte ensemble, par une PR dans chaque dépôt, guioxide compris.

```toml
[toolchain]
channel = "1.99.1"            # même valeur dans les 4 dépôts (octobre 2026)
components = ["rustfmt", "clippy"]
```

### 2. Backends : `wgpu` par défaut, `cuda` en option, CPU pour les tests

| Feature | Rôle | Où |
|---|---|---|
| `wgpu` (défaut) | Metal sur Mac, Vulkan sur Linux | tout le monde, tous les jours |
| `cuda` | performance sur la station NVIDIA | optionnel |
| backend CPU (`ndarray`) | oracle, tests en CI | toujours compilé |

Conséquence : medoxide doit pouvoir tourner sur la station (Vulkan ou CUDA), et surfoxide et
gyroxide sur le Mac (Metal). Les tests de CI tournent sur le backend CPU, puisque les
machines GitHub n'ont pas de GPU.

### 3. Burn : converger vers une même version stable

Pas bloquant pour guioxide, qui ne lie pas Burn. Mais trois versions dont une pré-version
font diverger les comportements (wgpu, CubeCL) entre outils. Cible : la même version stable
de Burn dans les trois outils, mise à jour ensemble.

### 4. CI identique dans les quatre dépôts

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest]
steps:
  - cargo fmt --all --check
  - cargo clippy --all-targets -- -D warnings
  - cargo test --release            # backend CPU
```

- Les tests qui exigent un GPU sont marqués `#[ignore = "gpu"]` et lancés à la main sur
  chaque machine : `cargo test --release -- --ignored`.
- Avant une version, on lance ces tests GPU sur les deux machines (Metal et CUDA/Vulkan).
- Plus tard, la station peut servir de *runner* auto-hébergé pour automatiser les tests GPU.

### 5. Un jeu de test commun

C'est ce qui permet de développer guioxide sans relancer toute la chaîne, et de vérifier que
le contrat de données tient.

- Un **petit jeu BIDS sans données de patiente** : fantôme numérique ou données publiques
  dont la licence permet la redistribution. 1 sujet, 1 session, 3 piles T2w.
- Il contient **les sorties de toutes les étapes** conformes au contrat : masques, volume
  SVR, `motion.tsv`, `slicequality.tsv`, segmentation, surfaces GIfTI, un court run gyroxide.
- Hébergé comme les poids de fetalbet (Hugging Face), avec un script de téléchargement et
  une somme SHA-256. Rien dans les dépôts git.
- Chaque outil a un test d'intégration (ignoré par défaut, lancé avant version) qui
  régénère sa part du jeu et la compare à la référence.

### 6. Conventions de code et de documentation

- Même `rustfmt.toml` (`max_width = 120`, réglage déjà en usage dans gyroxide) et mêmes lints
  clippy.
- Documentation et commentaires en français (déjà le cas), noms d'API en anglais ou en
  français mais cohérents dans un même crate.
- Chaque binaire accepte `--version` et écrit sa version dans `dataset_description.json`
  (`GeneratedBy`) de ses dérivées.
- Erreurs : messages destinés à l'utilisateur, avec le chemin du fichier en cause.

## guioxide sur les deux machines

- **Mac** : natif, Metal.
- **Linux** : natif, Vulkan.
- **Station RTX 2080 Ti** : Linux natif, Vulkan via le pilote NVIDIA. C'est la machine de
  référence Linux : guioxide y est essayé à chaque jalon, en plus de la CI.

## Ordre conseillé

1. Épingler la même toolchain et poser la même CI (une PR par dépôt).
2. Constituer le jeu de test.
3. Aligner les sorties sur le contrat (issues de [issues/](issues/)).
4. Converger les versions de Burn, quand un outil fait de toute façon une mise à jour.
