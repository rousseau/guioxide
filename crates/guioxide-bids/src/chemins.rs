//! Chemins des fichiers du contrat (`docs/donnees.md`) pour une session, dans un dossier `derivatives/`.

use std::path::{Path, PathBuf};

use crate::jeu::{Pile, Session, Sujet};

/// Dérivées d'une session : `<derivees>/<pipeline>/sub-X[/ses-Y]/anat/<prefixe>_…`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheminsSession {
    derivees: PathBuf,
    dossier: PathBuf,
    prefixe: String,
}

impl CheminsSession {
    pub fn new(derivees: &Path, sujet: &Sujet, session: &Session) -> CheminsSession {
        CheminsSession {
            derivees: derivees.to_path_buf(),
            dossier: session.dossier(sujet),
            prefixe: session.prefixe(sujet),
        }
    }

    /// Dossier `derivatives/` de la sortie choisie.
    pub fn derivees(&self) -> &Path {
        &self.derivees
    }

    /// Préfixe des noms de fichiers : `sub-X_ses-Y` ou `sub-X`.
    pub fn prefixe(&self) -> &str {
        &self.prefixe
    }

    /// Dossier `anat/` de la session dans les dérivées de `pipeline`.
    pub fn anat(&self, pipeline: &str) -> PathBuf {
        self.derivees.join(pipeline).join(&self.dossier).join("anat")
    }

    /// Masque cérébral d'une pile (`medx-fetalbet`), sur la grille de la pile.
    ///
    /// Nom BIDS écrit par medoxide : les entités de la pile sans son suffixe de contraste, puis `desc-brain_mask`
    /// (`sub-01_ses-01_acq-ax_run-1_T2w` → `sub-01_ses-01_acq-ax_run-1_desc-brain_mask.nii.gz`).
    pub fn masque(&self, pile: &Pile) -> PathBuf {
        let entites = pile
            .nom
            .strip_suffix(&format!("_{}", pile.contraste))
            .unwrap_or(&pile.nom);
        self.anat("medx-fetalbet")
            .join(format!("{entites}_desc-brain_mask.nii.gz"))
    }

    /// Volume reconstruit (`medx-svr`).
    pub fn svr(&self) -> PathBuf {
        self.anat("medx-svr")
            .join(format!("{}_desc-svr_T2w.nii.gz", self.prefixe))
    }

    /// Paramètres de la reconstruction (`.json` voisin du volume).
    pub fn parametres_svr(&self) -> PathBuf {
        self.anat("medx-svr")
            .join(format!("{}_desc-svr_T2w.json", self.prefixe))
    }

    /// Mouvement par coupe estimé par le SVR.
    pub fn mouvement(&self) -> PathBuf {
        self.anat("medx-svr")
            .join(format!("{}_desc-svr_motion.tsv", self.prefixe))
    }

    /// Qualité par coupe après recalage (SVR).
    pub fn qualite(&self) -> PathBuf {
        self.anat("medx-svr")
            .join(format!("{}_desc-svr_slicequality.tsv", self.prefixe))
    }

    /// Qualité des acquisitions avant reconstruction, si medoxide la calcule.
    pub fn qualite_acquisition(&self) -> PathBuf {
        self.anat("medx-svr")
            .join(format!("{}_desc-acq_slicequality.tsv", self.prefixe))
    }

    /// Segmentation (`medx-bounti`), sur la grille du volume reconstruit.
    pub fn segmentation(&self) -> PathBuf {
        self.anat("medx-bounti")
            .join(format!("{}_desc-bounti_dseg.nii.gz", self.prefixe))
    }

    /// Table des étiquettes de la segmentation (format BIDS `dseg.tsv`).
    pub fn etiquettes(&self) -> PathBuf {
        self.anat("medx-bounti")
            .join(format!("{}_desc-bounti_dseg.tsv", self.prefixe))
    }

    /// Surface `white` ou `pial` ; `hemi` : `Some("L")`, `Some("R")`, ou `None` pour les deux hémisphères.
    pub fn surface(&self, nom: &str, hemi: Option<&str>) -> PathBuf {
        let hemi = hemi.map(|h| format!("_hemi-{h}")).unwrap_or_default();
        self.anat("surfx-surfaces")
            .join(format!("{}{hemi}_{nom}.surf.gii", self.prefixe))
    }

    /// Champ par sommet (`curv`, `sulc`, `thickness`…) d'un hémisphère.
    pub fn champ(&self, nom: &str, hemi: Option<&str>) -> PathBuf {
        let hemi = hemi.map(|h| format!("_hemi-{h}")).unwrap_or_default();
        self.anat("surfx-surfaces")
            .join(format!("{}{hemi}_{nom}.shape.gii", self.prefixe))
    }

    /// Contrôle des maillages (une ligne par surface).
    pub fn controle_surfaces(&self) -> PathBuf {
        self.anat("surfx-surfaces")
            .join(format!("{}_desc-surfqc.tsv", self.prefixe))
    }

    /// Dossier `derivatives/guioxide` : les annotations de guioxide.
    pub fn racine_guioxide(&self) -> PathBuf {
        self.derivees.join("guioxide")
    }

    /// Validation par l'opérateur de la session.
    pub fn qc(&self) -> PathBuf {
        self.racine_guioxide()
            .join(&self.dossier)
            .join(format!("{}_desc-qc.json", self.prefixe))
    }

    /// Dossier des runs gyroxide (`run-*`).
    pub fn simulations(&self) -> PathBuf {
        self.anat("gyroxide")
    }
}
