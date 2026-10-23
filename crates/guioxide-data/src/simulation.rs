//! Runs de gyroxide (`docs/donnees.md`, étape 4) : `status.json`, `sequence.tsv`, dossiers `run-<label>`, lus par
//! `gyroxide-io` ; guioxide n'en fait qu'adapter les erreurs.

use std::path::{Path, PathBuf};

pub use gyroxide_io::{SequenceRow, State, Status};

use crate::ErreurDonnees;

/// `status.json` d'un run (`gyroxide_io::lire_status`).
pub fn lire_etat_run(chemin: &Path) -> Result<Status, ErreurDonnees> {
    gyroxide_io::lire_status(chemin).map_err(|e| ErreurDonnees::format(chemin, e))
}

/// `sequence.tsv` d'un run (`gyroxide_io::lire_sequence`).
pub fn lire_sequence(chemin: &Path) -> Result<Vec<SequenceRow>, ErreurDonnees> {
    gyroxide_io::lire_sequence(chemin).map_err(|e| ErreurDonnees::format(chemin, e))
}

/// Dossiers `run-<label>` d'un dossier de simulations, triés : `(label, chemin)` (`gyroxide_io::lister_runs`).
pub fn lister_runs(dossier: &Path) -> Vec<(String, PathBuf)> {
    gyroxide_io::lister_runs(dossier)
}
