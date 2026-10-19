//! `guioxide-bids` : indexation d'un jeu BIDS et de ses dérivées, état d'avancement par sujet et session.
//!
//! L'état d'une étape est déduit de la présence et de la validité des fichiers du contrat de données
//! (`docs/donnees.md`). Aucune dépendance graphique.
//!
mod avancement;
mod jeu;
mod tsv;

use std::fmt;
use std::path::PathBuf;

pub use avancement::{etat_etape, etats_session};
pub use jeu::{COLONNE_AGE, Derivee, JeuBids, Pile, Session, Sujet, indexer, lister_derivees, trouver_racine};
pub use tsv::Tableau;

/// Erreur de lecture d'un jeu : toujours avec le chemin en cause.
#[derive(Debug, thiserror::Error)]
pub enum ErreurBids {
    #[error("aucun jeu BIDS trouvé à partir de {0} (pas de dataset_description.json dans ce dossier ni au-dessus)")]
    RacineIntrouvable(PathBuf),
    #[error("{chemin} : {source}")]
    Lecture { chemin: PathBuf, source: std::io::Error },
    #[error("{chemin} : {message}")]
    Format { chemin: PathBuf, message: String },
}

impl ErreurBids {
    pub(crate) fn lecture(chemin: &std::path::Path, source: std::io::Error) -> Self {
        ErreurBids::Lecture {
            chemin: chemin.to_path_buf(),
            source,
        }
    }
}

/// Une étape de la chaîne, de l'examen acquis à la simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Etape {
    Examen,
    Reconstruction,
    Surfaces,
    Simulation,
}

impl Etape {
    /// Les étapes, dans l'ordre de la chaîne.
    pub const TOUTES: [Etape; 4] = [Etape::Examen, Etape::Reconstruction, Etape::Surfaces, Etape::Simulation];

    /// Numéro affiché (1 à 4), aussi raccourci clavier de l'onglet.
    pub fn numero(self) -> u8 {
        match self {
            Etape::Examen => 1,
            Etape::Reconstruction => 2,
            Etape::Surfaces => 3,
            Etape::Simulation => 4,
        }
    }

    /// Étape de numéro `n` (1 à 4).
    pub fn depuis_numero(n: u8) -> Option<Etape> {
        Etape::TOUTES.into_iter().find(|e| e.numero() == n)
    }

    pub fn nom(self) -> &'static str {
        match self {
            Etape::Examen => "Examen",
            Etape::Reconstruction => "Reconstruction",
            Etape::Surfaces => "Surfaces",
            Etape::Simulation => "Simulation",
        }
    }

    /// Outil qui produit les fichiers de l'étape.
    pub fn outil(self) -> &'static str {
        match self {
            Etape::Examen | Etape::Reconstruction => "medoxide",
            Etape::Surfaces => "surfoxide",
            Etape::Simulation => "gyroxide",
        }
    }
}

impl fmt::Display for Etape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.numero(), self.nom())
    }
}

/// État d'avancement d'une étape pour une session, déduit des fichiers présents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EtatEtape {
    #[default]
    NonFaite,
    EnCours,
    Terminee,
    /// Fichier présent mais illisible : message de lecture, avec le chemin en cause.
    Erreur(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeros_dans_l_ordre_de_la_chaine() {
        let numeros: Vec<u8> = Etape::TOUTES.iter().map(|e| e.numero()).collect();
        assert_eq!(numeros, [1, 2, 3, 4]);
        for e in Etape::TOUTES {
            assert_eq!(Etape::depuis_numero(e.numero()), Some(e));
        }
        assert_eq!(Etape::depuis_numero(0), None);
        assert_eq!(Etape::depuis_numero(5), None);
    }
}
