//! Sorties du SVR (`docs/donnees.md`, étapes 1 et 2) : qualité et mouvement par coupe, lus et calculés par
//! `medoxide_core::sorties` ; guioxide n'en fait qu'adapter les erreurs.

use std::path::Path;

pub use medoxide_core::sorties::{MouvementCoupe, PointMouvement, QualiteCoupe, mouvement_par_coupe};

use crate::ErreurDonnees;

fn erreur(chemin: &Path, e: medoxide_core::sorties::ErreurTableau) -> ErreurDonnees {
    ErreurDonnees::format(chemin, e)
}

/// `*_desc-svr_slicequality.tsv` (`medoxide_core::sorties::lire_qualite`).
pub fn lire_qualite(chemin: &Path) -> Result<Vec<QualiteCoupe>, ErreurDonnees> {
    medoxide_core::sorties::lire_qualite(chemin).map_err(|e| erreur(chemin, e))
}

/// `*_desc-svr_motion.tsv` (`medoxide_core::sorties::lire_mouvement`).
pub fn lire_mouvement(chemin: &Path) -> Result<Vec<MouvementCoupe>, ErreurDonnees> {
    medoxide_core::sorties::lire_mouvement(chemin).map_err(|e| erreur(chemin, e))
}
