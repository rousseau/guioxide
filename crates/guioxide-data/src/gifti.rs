//! Surfaces et champs par sommet, lus par `surfoxide-io` et mesurés par `surfoxide-core` ; guioxide n'en garde
//! qu'une forme d'affichage (sommets en `f32`).

use std::path::Path;

use surfoxide_core::{TopologyReport, topology_report};
pub use surfoxide_io::surfqc::SurfQc;

use crate::ErreurDonnees;

/// Un maillage triangulaire de surface, prêt à afficher.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Maillage {
    pub sommets: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
    /// `AnatomicalStructurePrimary` (`CortexLeft`, `CortexRight`…).
    pub structure: Option<String>,
    /// Topologie (sommets, arêtes, faces, caractéristique d'Euler, défauts), calculée par surfoxide.
    pub topologie: Option<TopologyReport>,
    /// Aire (mm²), calculée par surfoxide.
    pub aire_mm2: Option<f64>,
}

impl Maillage {
    /// Boîte englobante (min, max).
    pub fn boite(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for s in &self.sommets {
            for c in 0..3 {
                min[c] = min[c].min(s[c]);
                max[c] = max[c].max(s[c]);
            }
        }
        (min, max)
    }
}

/// Lit une surface GIfTI (`surfoxide_io::gifti::read_surface`) avec sa topologie et son aire.
pub fn lire_surface(chemin: &Path) -> Result<Maillage, ErreurDonnees> {
    let (maillage, meta) = surfoxide_io::gifti::read_surface(chemin).map_err(|e| ErreurDonnees::format(chemin, e))?;
    Ok(Maillage {
        sommets: maillage.vertices().iter().map(|v| v.map(|c| c as f32)).collect(),
        faces: maillage.faces().to_vec(),
        structure: meta.primary,
        topologie: Some(topology_report(&maillage)),
        aire_mm2: Some(maillage.area()),
    })
}

/// Contrôle des maillages d'une session (`*_desc-surfqc.tsv`), lu par `surfoxide_io::surfqc::read_surfqc`.
pub fn lire_controle_surfaces(chemin: &Path) -> Result<Vec<SurfQc>, ErreurDonnees> {
    surfoxide_io::surfqc::read_surfqc(chemin).map_err(|e| ErreurDonnees::format(chemin, e))
}

/// Lit le premier champ par sommet d'un fichier `.shape.gii` (`surfoxide_io::gifti::read_fields`).
pub fn lire_champ(chemin: &Path) -> Result<Vec<f32>, ErreurDonnees> {
    let champs = surfoxide_io::gifti::read_fields(chemin).map_err(|e| ErreurDonnees::format(chemin, e))?;
    let champ = champs
        .into_iter()
        .next()
        .ok_or_else(|| ErreurDonnees::format(chemin, "aucun champ"))?;
    Ok(champ.values.into_iter().map(|v| v as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use surfoxide_core::{TriMesh, VertexField};
    use surfoxide_io::gifti::{SurfaceMeta, write_fields, write_surface};

    /// Aller-retour par surfoxide-io : la conversion d'affichage garde sommets, faces, structure, et ajoute les mesures.
    #[test]
    fn surface_et_champ_lus_par_surfoxide() {
        let dir = tempfile::tempdir().unwrap();
        let sommets = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let tetra = TriMesh::new(sommets, faces.clone()).unwrap();
        let meta = SurfaceMeta::hemisphere(true, "Pial");
        let s = dir.path().join("t.surf.gii");
        write_surface(&s, &tetra, &meta).unwrap();
        let m = lire_surface(&s).unwrap();
        assert_eq!(m.sommets[1], [1.0, 0.0, 0.0]);
        assert_eq!(m.faces, faces);
        assert_eq!(m.structure.as_deref(), Some("CortexLeft"));
        assert_eq!(m.topologie.as_ref().map(|t| t.euler_characteristic), Some(2));
        assert!((m.aire_mm2.unwrap() - (1.5 + 3f64.sqrt() / 2.0)).abs() < 1e-9);

        let c = dir.path().join("t.shape.gii");
        write_fields(&c, &[VertexField::new("thickness", vec![1.0, 2.0, 3.5, 4.0])], &meta).unwrap();
        assert_eq!(lire_champ(&c).unwrap(), [1.0, 2.0, 3.5, 4.0]);
        assert!(
            lire_surface(&dir.path().join("absent.surf.gii"))
                .unwrap_err()
                .to_string()
                .contains("absent.surf.gii")
        );
    }
}
