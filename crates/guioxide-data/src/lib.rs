//! `guioxide-data` : chargement des données lues par guioxide.
//!
//! - Volumes NIfTI, lus par `medoxide-core` (voxels et affine par la même règle sform / qform que les outils) ;
//! - tableaux TSV (qualité, poses, étiquettes) ;
//! - [`Chargeur`] : lecture sur des fils d'arrière-plan, avec un cache évincé par taille.
//!
//! Aucune dépendance graphique.

mod chargeur;
mod etiquettes;
mod svr;
mod tsv;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use guioxide_geom::{Affine, Volume};

pub use chargeur::{Chargeur, Etat};
pub use etiquettes::{Etiquette, TableEtiquettes, couleur_par_defaut};
pub use svr::{MouvementCoupe, PointMouvement, QualiteCoupe, lire_mouvement, lire_qualite, mouvement_par_coupe};
pub use tsv::Tableau;

/// Erreur de lecture d'un fichier : toujours avec le chemin en cause.
#[derive(Debug, thiserror::Error)]
pub enum ErreurDonnees {
    #[error("{chemin} : {source}")]
    Lecture { chemin: PathBuf, source: std::io::Error },
    #[error("{chemin} : {message}")]
    Format { chemin: PathBuf, message: String },
}

impl ErreurDonnees {
    pub(crate) fn lecture(chemin: &Path, source: std::io::Error) -> Self {
        ErreurDonnees::Lecture {
            chemin: chemin.to_path_buf(),
            source,
        }
    }

    pub(crate) fn format(chemin: &Path, message: impl ToString) -> Self {
        ErreurDonnees::Format {
            chemin: chemin.to_path_buf(),
            message: message.to_string(),
        }
    }
}

/// Un volume lu, avec l'origine de sa géométrie.
#[derive(Debug)]
pub struct VolumeLu {
    pub volume: Arc<Volume>,
    /// Champ de l'en-tête dont l'affine est tirée : `sform`, `qform` ou `pixdim`.
    pub source_affine: &'static str,
}

/// Géométrie d'un volume NIfTI, lue dans l'en-tête seul : dimensions et affine (sform, sinon qform, sinon pixdim).
pub fn lire_geometrie(chemin: &Path) -> Result<([usize; 3], Affine), ErreurDonnees> {
    let info = medoxide_core::volume_info(chemin).map_err(|e| ErreurDonnees::format(chemin, e))?;
    let affine = medoxide_core::affine_nifti(chemin).map_err(|e| ErreurDonnees::format(chemin, e))?;
    match info.dim[..] {
        [x, y, z] => Ok(([x as usize, y as usize, z as usize], Affine(affine.matrice))),
        _ => Err(ErreurDonnees::format(
            chemin,
            format!("volume 3D attendu, dimensions {:?}", info.dim),
        )),
    }
}

/// Lit un volume NIfTI (`.nii` ou `.nii.gz`) : voxels en `f32` et affine (sform, sinon qform, sinon pixdim).
pub fn lire_volume(chemin: &Path) -> Result<VolumeLu, ErreurDonnees> {
    let affine = medoxide_core::affine_nifti(chemin).map_err(|e| ErreurDonnees::format(chemin, e))?;
    let voxels = medoxide_core::read_volume(chemin).map_err(|e| ErreurDonnees::format(chemin, e))?;
    let forme = voxels.shape();
    let dims = [forme[0], forme[1], forme[2]];
    // Axes inversés : l'itération logique devient i le plus rapide, l'ordre de `Volume`. Les volumes NIfTI sont en
    // ordre Fortran : la mémoire est alors déjà dans cet ordre et rien n'est copié.
    let inverse = voxels.reversed_axes();
    let voxels = if inverse.is_standard_layout() {
        inverse.into_raw_vec_and_offset().0
    } else {
        inverse.iter().copied().collect()
    };
    let volume = Volume::new(dims, Affine(affine.matrice), voxels).map_err(|e| ErreurDonnees::format(chemin, e))?;
    let source_affine = match affine.source {
        medoxide_core::SourceAffine::Sform => "sform",
        medoxide_core::SourceAffine::Qform => "qform",
        medoxide_core::SourceAffine::Pixdim => "pixdim",
    };
    Ok(VolumeLu {
        volume: Arc::new(volume),
        source_affine,
    })
}
