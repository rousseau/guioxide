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

    /// Dossier `anat/` de la session dans les dérivées de `pipeline`.
    pub fn anat(&self, pipeline: &str) -> PathBuf {
        self.derivees.join(pipeline).join(&self.dossier).join("anat")
    }

    /// Masque cérébral d'une pile (`medx-fetalbet`), sur la grille de la pile.
    pub fn masque(&self, pile: &Pile) -> PathBuf {
        self.anat("medx-fetalbet")
            .join(format!("{}_desc-brain_mask.nii.gz", pile.nom))
    }

    /// Volume reconstruit (`medx-svr`).
    pub fn svr(&self) -> PathBuf {
        self.anat("medx-svr")
            .join(format!("{}_desc-svr_T2w.nii.gz", self.prefixe))
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

    /// Dossier des runs gyroxide (`run-*`).
    pub fn simulations(&self) -> PathBuf {
        self.anat("gyroxide")
    }
}
