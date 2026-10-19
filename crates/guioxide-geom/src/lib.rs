//! `guioxide-geom` : repères de guioxide.
//!
//! Repère monde : **RAS, en mm** (convention NIfTI), commun aux volumes et aux maillages. Toutes les vues partagent un
//! curseur exprimé dans ce repère, jamais en indices de voxel (voir `docs/architecture.md`) : c'est ce qui lie des
//! volumes de résolutions et d'orientations différentes.
//!
//! - [`Affine`] : voxel → monde (règle sform / qform appliquée à la lecture, par `medoxide-core`).
//! - [`Volume`] : grille de voxels et son affine ; échantillonnage trilinéaire (image) ou au plus proche (étiquettes).
//! - [`Plan`] : plan de coupe dans le monde ; [`Plan::echantillonner`] en tire une image 2D.

mod plan;

pub use plan::{Coupe, Interpolation, Orientation, Plan};

/// Point du repère monde : RAS, en millimètres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointMonde(pub [f64; 3]);

/// Transformation affine 4×4, rangée ligne par ligne (dernière ligne `0 0 0 1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [[f64; 4]; 4]);

impl Affine {
    pub const IDENTITE: Affine = Affine([[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], [0., 0., 0., 1.]]);

    pub fn appliquer(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
    }

    /// Colonne `c` de la partie linéaire : direction et longueur (mm) de l'axe d'indice `c` dans le monde.
    pub fn colonne(&self, c: usize) -> [f64; 3] {
        [self.0[0][c], self.0[1][c], self.0[2][c]]
    }

    /// Longueur des colonnes de la partie linéaire : taille des voxels le long de chaque axe d'indice (mm).
    pub fn tailles(&self) -> [f64; 3] {
        std::array::from_fn(|c| norme(self.colonne(c)))
    }

    /// Valeur absolue du déterminant de la partie linéaire : volume d'un voxel (mm³).
    pub fn volume_voxel(&self) -> f64 {
        let m = &self.0;
        (m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]))
            .abs()
    }

    /// Angle (degrés) de la rotation d'une transformation rigide : `acos((trace(R) − 1) / 2)`.
    pub fn angle_rotation(&self) -> f64 {
        let trace = self.0[0][0] + self.0[1][1] + self.0[2][2];
        ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees()
    }

    /// Déplacement (mm) du point `p` par la transformation : `‖T(p) − p‖`.
    pub fn deplacement(&self, p: [f64; 3]) -> f64 {
        let q = self.appliquer(p);
        norme(std::array::from_fn(|c| q[c] - p[c]))
    }

    /// Inverse d'une affine ; `None` si la partie linéaire est singulière.
    pub fn inverse(&self) -> Option<Affine> {
        let m = &self.0;
        let a = [
            [m[0][0], m[0][1], m[0][2]],
            [m[1][0], m[1][1], m[1][2]],
            [m[2][0], m[2][1], m[2][2]],
        ];
        let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        // Inverse de la partie linéaire par les cofacteurs, puis translation −A⁻¹·t.
        let cof = |r: usize, c: usize| {
            let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
            let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
            a[r1][c1] * a[r2][c2] - a[r1][c2] * a[r2][c1]
        };
        let inv: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| cof(c, r) / det));
        let t = [m[0][3], m[1][3], m[2][3]];
        let mut sortie = Affine::IDENTITE;
        for (ligne, i) in sortie.0.iter_mut().zip(inv) {
            ligne[..3].copy_from_slice(&i);
            ligne[3] = -(i[0] * t[0] + i[1] * t[1] + i[2] * t[2]);
        }
        Some(sortie)
    }
}

/// Erreur de construction d'un volume.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ErreurGeom {
    #[error("affine singulière : le volume n'a pas de géométrie exploitable")]
    AffineSinguliere,
    #[error("{voxels} voxels pour une grille {dims:?}")]
    TailleIncoherente { dims: [usize; 3], voxels: usize },
}

/// Un volume scalaire : voxels (indice `i + nx·(j + ny·k)`) et affine voxel → monde.
#[derive(Clone, Debug)]
pub struct Volume {
    dims: [usize; 3],
    affine: Affine,
    inverse: Affine,
    voxels: Vec<f32>,
    fenetre_auto: std::sync::OnceLock<(f32, f32)>,
}

impl Volume {
    pub fn new(dims: [usize; 3], affine: Affine, voxels: Vec<f32>) -> Result<Volume, ErreurGeom> {
        if voxels.len() != dims.iter().product::<usize>() {
            return Err(ErreurGeom::TailleIncoherente {
                dims,
                voxels: voxels.len(),
            });
        }
        let inverse = affine.inverse().ok_or(ErreurGeom::AffineSinguliere)?;
        Ok(Volume {
            dims,
            affine,
            inverse,
            voxels,
            fenetre_auto: Default::default(),
        })
    }

    pub fn dims(&self) -> [usize; 3] {
        self.dims
    }

    pub fn affine(&self) -> &Affine {
        &self.affine
    }

    pub fn voxels(&self) -> &[f32] {
        &self.voxels
    }

    /// Taille mémoire des voxels, en octets (pour le cache).
    pub fn octets(&self) -> usize {
        self.voxels.len() * std::mem::size_of::<f32>()
    }

    /// Taille des voxels le long de chaque axe d'indice (mm).
    pub fn taille_voxel(&self) -> [f64; 3] {
        self.affine.tailles()
    }

    pub fn valeur(&self, i: usize, j: usize, k: usize) -> f32 {
        self.voxels[i + self.dims[0] * (j + self.dims[1] * k)]
    }

    /// Indices continus (centre du voxel `(i, j, k)` = valeurs entières) d'un point monde.
    pub fn vers_voxel(&self, p: [f64; 3]) -> [f64; 3] {
        self.inverse.appliquer(p)
    }

    pub fn vers_monde(&self, ijk: [f64; 3]) -> [f64; 3] {
        self.affine.appliquer(ijk)
    }

    /// Centre de la grille, dans le monde.
    pub fn centre(&self) -> PointMonde {
        PointMonde(self.vers_monde(std::array::from_fn(|c| (self.dims[c] as f64 - 1.0) / 2.0)))
    }

    /// Boîte englobante (min, max) dans le monde, bords des voxels compris.
    pub fn boite(&self) -> ([f64; 3], [f64; 3]) {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for coin in 0..8 {
            let ijk: [f64; 3] = std::array::from_fn(|c| {
                if coin >> c & 1 == 0 {
                    -0.5
                } else {
                    self.dims[c] as f64 - 0.5
                }
            });
            let p = self.vers_monde(ijk);
            for c in 0..3 {
                min[c] = min[c].min(p[c]);
                max[c] = max[c].max(p[c]);
            }
        }
        (min, max)
    }

    /// Interpolation trilinéaire ; `None` hors de la grille (au-delà des centres des voxels du bord).
    pub fn trilineaire(&self, p: [f64; 3]) -> Option<f32> {
        self.trilineaire_voxel(self.vers_voxel(p))
    }

    /// Comme [`Volume::trilineaire`], en indices continus.
    pub fn trilineaire_voxel(&self, x: [f64; 3]) -> Option<f32> {
        let mut base = [0usize; 3];
        let mut frac = [0f64; 3];
        for c in 0..3 {
            let n = self.dims[c];
            // Tolérance d'arrondi : un point posé sur un voxel du bord revient de l'aller-retour par l'affine à
            // ±1e-12 près.
            if !(x[c] >= -BORD && x[c] <= (n - 1) as f64 + BORD) {
                return None;
            }
            let xc = x[c].clamp(0.0, (n - 1) as f64);
            // Le dernier voxel est pris comme base de la cellule précédente, poids 1 sur lui.
            let b = (xc.floor() as usize).min(n.saturating_sub(2));
            base[c] = b;
            frac[c] = xc - b as f64;
        }
        let mut somme = 0.0;
        for coin in 0..8usize {
            let mut poids = 1.0;
            let mut idx = [0usize; 3];
            for c in 0..3 {
                let haut = coin >> c & 1;
                if self.dims[c] == 1 {
                    if haut == 1 {
                        poids = 0.0;
                    }
                    idx[c] = 0;
                    continue;
                }
                idx[c] = base[c] + haut;
                poids *= if haut == 1 { frac[c] } else { 1.0 - frac[c] };
            }
            if poids != 0.0 {
                somme += poids * self.valeur(idx[0], idx[1], idx[2]) as f64;
            }
        }
        Some(somme as f32)
    }

    /// Valeur du voxel le plus proche (étiquettes, masques) ; `None` hors de la grille (bords des voxels compris).
    pub fn plus_proche(&self, p: [f64; 3]) -> Option<f32> {
        self.plus_proche_voxel(self.vers_voxel(p))
    }

    /// Comme [`Volume::plus_proche`], en indices continus.
    pub fn plus_proche_voxel(&self, x: [f64; 3]) -> Option<f32> {
        let mut idx = [0usize; 3];
        for c in 0..3 {
            let r = x[c].round();
            if !(r >= 0.0 && r < self.dims[c] as f64) {
                return None;
            }
            idx[c] = r as usize;
        }
        Some(self.valeur(idx[0], idx[1], idx[2]))
    }

    /// Fenêtre automatique `(niveau, largeur)` : centiles 1 % et 99 % des voxels non nuls, calculés une fois.
    pub fn fenetre_auto(&self) -> (f32, f32) {
        *self.fenetre_auto.get_or_init(|| {
            let (a, b) = self.centiles(0.01, 0.99);
            ((a + b) / 2.0, (b - a).max(1e-6))
        })
    }

    /// Valeurs aux centiles `bas` et `haut` (dans [0, 1]) des voxels non nuls, sur un échantillon régulier :
    /// fenêtrage automatique.
    pub fn centiles(&self, bas: f64, haut: f64) -> (f32, f32) {
        let pas = (self.voxels.len() / 200_000).max(1);
        let mut v: Vec<f32> = self
            .voxels
            .iter()
            .step_by(pas)
            .copied()
            .filter(|x| x.is_finite() && *x != 0.0)
            .collect();
        if v.is_empty() {
            return (0.0, 1.0);
        }
        v.sort_by(f32::total_cmp);
        let rang = |q: f64| v[((v.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize];
        (rang(bas), rang(haut))
    }
}

/// Tolérance (en indices) aux bords de la grille pour l'interpolation trilinéaire.
const BORD: f64 = 1e-6;

pub(crate) fn norme(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Affine d'une vraie pile fœtale (fetus_03 du jeu de développement de medoxide), lue par nibabel :
    /// oblique, voxels anisotropes, déterminant positif mais axe x inversé.
    pub(crate) const PILE_OBLIQUE: Affine = Affine([
        [-1.171875, 0.0, 0.0, 155.1199188232],
        [0.0, -1.077111721, 0.7878836393, 58.3841590881],
        [0.0, 0.4616506398, 1.8382704258, -254.6114807129],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    fn proches(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|c| (a[c] - b[c]).abs() <= tol)
    }

    #[test]
    fn voxel_monde_comme_nibabel_sur_une_pile_oblique() {
        // Références : nibabel, `img.affine @ [i, j, k, 1]`.
        let cas = [
            ([0.0, 0.0, 0.0], [155.1199188232, 58.3841590881, -254.6114807129]),
            ([10.5, 20.25, 3.0], [142.8152313232, 38.9362976551, -239.7482439801]),
            ([255.0, 255.0, 43.0], [-143.7082061768, -182.4003332853, -57.8449392617]),
        ];
        let inverse = PILE_OBLIQUE.inverse().unwrap();
        for (ijk, monde) in cas {
            assert!(proches(PILE_OBLIQUE.appliquer(ijk), monde, 1e-6), "{ijk:?}");
            assert!(proches(inverse.appliquer(monde), ijk, 1e-6), "{ijk:?}");
        }
    }

    #[test]
    fn mesures_d_une_transformation_rigide() {
        // Rotation de 30° autour de z, puis translation (1, 2, 2).
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let m = Affine([
            [c, -s, 0.0, 1.0],
            [s, c, 0.0, 2.0],
            [0.0, 0.0, 1.0, 2.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        assert!((m.angle_rotation() - 30.0).abs() < 1e-9);
        assert!((m.deplacement([0.0, 0.0, 0.0]) - 3.0).abs() < 1e-12);
        assert_eq!(Affine::IDENTITE.angle_rotation(), 0.0);
        assert_eq!(Affine::IDENTITE.deplacement([4.0, 5.0, 6.0]), 0.0);
    }

    #[test]
    fn affine_singuliere_refusee() {
        let mut a = Affine::IDENTITE;
        a.0[2][2] = 0.0;
        assert_eq!(a.inverse(), None);
        assert!(matches!(
            Volume::new([1, 1, 1], a, vec![0.0]),
            Err(ErreurGeom::AffineSinguliere)
        ));
        assert!(matches!(
            Volume::new([2, 2, 2], Affine::IDENTITE, vec![0.0]),
            Err(ErreurGeom::TailleIncoherente { .. })
        ));
    }

    /// Volume dont l'intensité est une fonction affine de la position monde : l'interpolation trilinéaire la
    /// reproduit exactement en tout point intérieur, quelle que soit l'affine.
    pub(crate) fn volume_lineaire(dims: [usize; 3], affine: Affine) -> (Volume, impl Fn([f64; 3]) -> f64) {
        let f = |p: [f64; 3]| 3.0 + 0.5 * p[0] - 0.25 * p[1] + 0.125 * p[2];
        let mut voxels = Vec::with_capacity(dims.iter().product());
        for k in 0..dims[2] {
            for j in 0..dims[1] {
                for i in 0..dims[0] {
                    voxels.push(f(affine.appliquer([i as f64, j as f64, k as f64])) as f32);
                }
            }
        }
        (Volume::new(dims, affine, voxels).unwrap(), f)
    }

    #[test]
    fn trilineaire_exacte_sur_une_fonction_lineaire_et_une_grille_oblique() {
        let (vol, f) = volume_lineaire([20, 18, 7], PILE_OBLIQUE);
        for ijk in [[3.3, 4.7, 2.5], [0.0, 0.0, 0.0], [19.0, 17.0, 6.0], [12.01, 0.99, 5.5]] {
            let p = vol.vers_monde(ijk);
            let v = vol.trilineaire(p).unwrap() as f64;
            assert!(
                (v - f(p)).abs() < 1e-3 * f(p).abs().max(1.0),
                "{ijk:?} : {v} ≠ {}",
                f(p)
            );
        }
        assert_eq!(vol.trilineaire(vol.vers_monde([-0.01, 3.0, 3.0])), None);
        assert_eq!(vol.trilineaire(vol.vers_monde([5.0, 3.0, 6.01])), None);
    }

    #[test]
    fn plus_proche_voisin_et_bords() {
        let voxels: Vec<f32> = (0..8).map(|v| v as f32).collect();
        let vol = Volume::new([2, 2, 2], PILE_OBLIQUE, voxels).unwrap();
        assert_eq!(vol.plus_proche(vol.vers_monde([0.4, 0.6, 0.2])), Some(2.0)); // (0, 1, 0)
        assert_eq!(vol.plus_proche(vol.vers_monde([1.49, 1.0, 1.0])), Some(7.0));
        assert_eq!(vol.plus_proche(vol.vers_monde([1.51, 1.0, 1.0])), None);
        assert_eq!(vol.plus_proche(vol.vers_monde([-0.49, 0.0, 0.0])), Some(0.0));
    }

    #[test]
    fn boite_centre_et_tailles() {
        let mut a = Affine::IDENTITE;
        (a.0[0][0], a.0[1][1], a.0[2][2]) = (0.5, 0.5, 2.0);
        a.0[0][3] = -10.0;
        let vol = Volume::new([4, 2, 3], a, vec![0.0; 24]).unwrap();
        assert_eq!(vol.taille_voxel(), [0.5, 0.5, 2.0]);
        assert_eq!(vol.affine().volume_voxel(), 0.5);
        assert_eq!(vol.centre(), PointMonde([-9.25, 0.25, 2.0]));
        assert_eq!(vol.boite(), ([-10.25, -0.25, -1.0], [-8.25, 0.75, 5.0]));
    }

    #[test]
    fn centiles_ignorent_les_zeros() {
        let mut voxels = vec![0.0; 50];
        voxels.extend((1..=101).map(|v| v as f32));
        let vol = Volume::new([151, 1, 1], Affine::IDENTITE, voxels).unwrap();
        assert_eq!(vol.centiles(0.0, 1.0), (1.0, 101.0));
        assert_eq!(vol.centiles(0.5, 0.99), (51.0, 100.0));
    }
}
