//! Plans de coupe dans le monde et échantillonnage d'un volume sur un plan.
//!
//! Convention d'affichage **radiologique** : la droite de la patiente est à gauche de l'écran. Les vues affichent les
//! lettres d'orientation (R/L, A/P, S/I) sur leurs bords.

use crate::{Affine, Volume, norme};

/// Orientation d'une vue de coupe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Plan z constant : écran → gauche de la patiente (L), écran ↑ antérieur (A).
    Axiale,
    /// Plan y constant : écran → L, écran ↑ supérieur (S).
    Coronale,
    /// Plan x constant : écran → postérieur (P), écran ↑ S.
    Sagittale,
}

impl Orientation {
    pub const TOUTES: [Orientation; 3] = [Orientation::Axiale, Orientation::Coronale, Orientation::Sagittale];

    /// Orientation des coupes d'une pile : axe monde dominant de la normale à ses coupes (`col0 × col1`).
    pub fn des_coupes(affine: &Affine) -> Orientation {
        let (a, b) = (affine.colonne(0), affine.colonne(1));
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        match (0..3).max_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs())) {
            Some(0) => Orientation::Sagittale,
            Some(1) => Orientation::Coronale,
            _ => Orientation::Axiale,
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Orientation::Axiale => "axiale",
            Orientation::Coronale => "coronale",
            Orientation::Sagittale => "sagittale",
        }
    }
}

/// Plan de coupe : origine (un point monde) et deux directions orthonormées, `u` vers la droite de l'écran, `v` vers
/// le haut. Les coordonnées dans le plan `(a, b)` sont en mm : point monde = origine + a·u + b·v.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub origine: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
}

/// Une coupe échantillonnée : image 2D, lignes de haut en bas ; `NaN` hors du volume.
#[derive(Clone, Debug)]
pub struct Coupe {
    pub largeur: usize,
    pub hauteur: usize,
    /// Taille d'un pixel, en mm.
    pub pas: f64,
    pub valeurs: Vec<f32>,
}

/// Mode d'interpolation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    /// Images d'intensité.
    Trilineaire,
    /// Étiquettes et masques.
    PlusProche,
}

impl Plan {
    /// Plan aligné sur les axes du monde, passant par `origine`.
    pub fn aligne(orientation: Orientation, origine: [f64; 3]) -> Plan {
        let (u, v) = match orientation {
            Orientation::Axiale => ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            Orientation::Coronale => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            Orientation::Sagittale => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        };
        Plan { origine, u, v }
    }

    /// Plan natif d'une pile : celui de ses coupes (axes d'indice `i` et `j`), passant par `origine`.
    /// `u` suit l'axe `i`, `v` la composante de l'axe `j` orthogonale à `u`.
    pub fn natif(affine: &Affine, origine: [f64; 3]) -> Plan {
        let u = normaliser(affine.colonne(0));
        let j = affine.colonne(1);
        let d = produit_scalaire(j, u);
        let v = normaliser(std::array::from_fn(|c| j[c] - d * u[c]));
        Plan { origine, u, v }
    }

    /// Normale du plan, `u × v`.
    pub fn normale(&self) -> [f64; 3] {
        let (u, v) = (self.u, self.v);
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    }

    pub fn vers_monde(&self, a: f64, b: f64) -> [f64; 3] {
        std::array::from_fn(|c| self.origine[c] + a * self.u[c] + b * self.v[c])
    }

    /// Coordonnées dans le plan de la projection orthogonale de `p`.
    pub fn vers_plan(&self, p: [f64; 3]) -> (f64, f64) {
        let d: [f64; 3] = std::array::from_fn(|c| p[c] - self.origine[c]);
        (produit_scalaire(d, self.u), produit_scalaire(d, self.v))
    }

    /// Distance signée de `p` au plan, le long de la normale.
    pub fn distance(&self, p: [f64; 3]) -> f64 {
        let d: [f64; 3] = std::array::from_fn(|c| p[c] - self.origine[c]);
        produit_scalaire(d, self.normale())
    }

    /// Échantillonne `volume` sur une fenêtre `largeur` × `hauteur` pixels de `pas` mm, centrée en `centre`
    /// (coordonnées dans le plan). Le pixel `(x, y)` (y vers le bas) a pour centre
    /// `a = centre.0 + (x + ½ − largeur/2)·pas`, `b = centre.1 − (y + ½ − hauteur/2)·pas`.
    pub fn echantillonner(
        &self,
        volume: &Volume,
        centre: (f64, f64),
        largeur: usize,
        hauteur: usize,
        pas: f64,
        interpolation: Interpolation,
    ) -> Coupe {
        // Tout se fait en indices continus : x(a, b) = A⁻¹·origine + a·A⁻¹u + b·A⁻¹v, linéaire en (a, b).
        let inv = |p: [f64; 3]| volume.vers_voxel(p);
        let o = inv(self.origine);
        let du: [f64; 3] = std::array::from_fn(|c| inv(self.vers_monde(1.0, 0.0))[c] - o[c]);
        let dv: [f64; 3] = std::array::from_fn(|c| inv(self.vers_monde(0.0, 1.0))[c] - o[c]);
        let mut valeurs = Vec::with_capacity(largeur * hauteur);
        for y in 0..hauteur {
            let b = centre.1 - (y as f64 + 0.5 - hauteur as f64 / 2.0) * pas;
            for x in 0..largeur {
                let a = centre.0 + (x as f64 + 0.5 - largeur as f64 / 2.0) * pas;
                let ijk: [f64; 3] = std::array::from_fn(|c| o[c] + a * du[c] + b * dv[c]);
                let v = match interpolation {
                    Interpolation::Trilineaire => volume.trilineaire_voxel(ijk),
                    Interpolation::PlusProche => volume.plus_proche_voxel(ijk),
                };
                valeurs.push(v.unwrap_or(f32::NAN));
            }
        }
        Coupe {
            largeur,
            hauteur,
            pas,
            valeurs,
        }
    }
}

fn produit_scalaire(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normaliser(a: [f64; 3]) -> [f64; 3] {
    let n = norme(a);
    a.map(|x| x / n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{PILE_OBLIQUE, volume_lineaire};

    #[test]
    fn orientation_des_coupes_d_une_pile() {
        assert_eq!(Orientation::des_coupes(&Affine::IDENTITE), Orientation::Axiale);
        // Pile oblique réelle : coupes proches du plan axial (normale surtout selon z).
        assert_eq!(Orientation::des_coupes(&PILE_OBLIQUE), Orientation::Axiale);
        let sagittale = Affine([
            [0.0, 0.0, 3.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        assert_eq!(Orientation::des_coupes(&sagittale), Orientation::Sagittale);
    }

    #[test]
    fn plans_alignes_convention_radiologique() {
        let p = Plan::aligne(Orientation::Axiale, [1.0, 2.0, 3.0]);
        assert_eq!(p.normale(), [0.0, 0.0, -1.0]);
        assert_eq!(
            p.vers_monde(1.0, 0.0),
            [0.0, 2.0, 3.0],
            "écran → : vers la gauche de la patiente (−x)"
        );
        assert_eq!(p.vers_plan([0.0, 4.0, 10.0]), (1.0, 2.0));
        assert_eq!(p.distance([0.0, 4.0, 10.0]), -7.0);
        assert_eq!(
            Plan::aligne(Orientation::Coronale, [0.0; 3]).vers_monde(0.0, 1.0),
            [0.0, 0.0, 1.0]
        );
        assert_eq!(
            Plan::aligne(Orientation::Sagittale, [0.0; 3]).vers_monde(1.0, 0.0),
            [0.0, -1.0, 0.0]
        );
    }

    #[test]
    fn echantillonnage_exact_sur_un_plan_oblique() {
        // Volume oblique, fonction linéaire du monde, coupé par un plan coronal du monde (oblique pour la grille).
        let (vol, f) = volume_lineaire([60, 60, 30], PILE_OBLIQUE);
        let centre_vol = vol.centre().0;
        let plan = Plan::aligne(Orientation::Coronale, centre_vol);
        let coupe = plan.echantillonner(&vol, (0.0, 0.0), 31, 21, 0.7, Interpolation::Trilineaire);
        let mut interieurs = 0;
        for y in 0..coupe.hauteur {
            for x in 0..coupe.largeur {
                let v = coupe.valeurs[y * coupe.largeur + x];
                if v.is_nan() {
                    continue;
                }
                interieurs += 1;
                let a = (x as f64 + 0.5 - 31.0 / 2.0) * 0.7;
                let b = -(y as f64 + 0.5 - 21.0 / 2.0) * 0.7;
                let attendu = f(plan.vers_monde(a, b));
                assert!(
                    (v as f64 - attendu).abs() < 1e-3 * attendu.abs().max(1.0),
                    "({x}, {y}) : {v} ≠ {attendu}"
                );
            }
        }
        assert!(
            interieurs > 31 * 21 / 2,
            "la coupe passe par le centre : la plupart des pixels sont dedans"
        );
    }

    #[test]
    fn plan_natif_retrouve_les_voxels_d_une_coupe() {
        // Une coupe native de la pile, centrée sur un voxel, au pas du voxel : chaque pixel tombe sur un centre.
        let dims = [8, 6, 5];
        let voxels: Vec<f32> = (0..dims.iter().product::<usize>()).map(|v| v as f32).collect();
        let vol = Volume::new(dims, PILE_OBLIQUE, voxels).unwrap();
        let k = 2;
        let plan = Plan::natif(vol.affine(), vol.vers_monde([0.0, 0.0, k as f64]));
        assert!(
            plan.distance(vol.vers_monde([5.0, 3.0, k as f64])).abs() < 1e-9,
            "les voxels de la coupe sont dans le plan"
        );
        let pas = vol.taille_voxel()[0];
        assert!(
            (vol.taille_voxel()[1] - 1.171875).abs() < 1e-6,
            "pile à voxels carrés dans le plan"
        );
        let coupe = plan.echantillonner(&vol, (3.5 * pas, 2.5 * pas), 8, 6, pas, Interpolation::PlusProche);
        // Écran ↑ = composante de l'axe j : la ligne du haut est j maximal.
        for y in 0..6 {
            for x in 0..8 {
                let (i, j) = (x, 5 - y);
                let attendu = vol.valeur(i, j, k);
                let lu = coupe.valeurs[y * 8 + x];
                assert_eq!(lu, attendu, "pixel ({x}, {y}) → voxel ({i}, {j}, {k})");
            }
        }
    }
}
