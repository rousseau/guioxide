//! Palettes de couleurs des champs par sommet : séquentielle (épaisseur) et divergente (courbure, profondeur).

/// Points d'appui de la palette séquentielle (viridis, 5 couleurs) : du violet foncé au jaune.
const SEQUENTIELLE: [[f32; 3]; 5] = [
    [68.0, 1.0, 84.0],
    [59.0, 82.0, 139.0],
    [33.0, 145.0, 140.0],
    [94.0, 201.0, 98.0],
    [253.0, 231.0, 37.0],
];

/// Palette divergente : bleu, gris clair au centre, rouge.
const DIVERGENTE: [[f32; 3]; 3] = [[49.0, 99.0, 206.0], [220.0, 220.0, 220.0], [206.0, 61.0, 49.0]];

fn interpoler(points: &[[f32; 3]], t: f32) -> [u8; 3] {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.5 };
    let x = t * (points.len() - 1) as f32;
    let i = (x.floor() as usize).min(points.len() - 2);
    let f = x - i as f32;
    std::array::from_fn(|c| (points[i][c] * (1.0 - f) + points[i + 1][c] * f).round() as u8)
}

/// `t` ∈ [0, 1] → couleur séquentielle.
pub fn sequentielle(t: f32) -> [u8; 3] {
    interpoler(&SEQUENTIELLE, t)
}

/// `t` ∈ [0, 1] → couleur divergente (½ au centre).
pub fn divergente(t: f32) -> [u8; 3] {
    interpoler(&DIVERGENTE, t)
}

/// Bornes robustes d'un champ : centiles 2 % et 98 % des valeurs finies.
pub fn bornes(valeurs: &[f32]) -> (f32, f32) {
    let mut v: Vec<f32> = valeurs.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return (0.0, 1.0);
    }
    v.sort_by(f32::total_cmp);
    let q = |p: f32| v[((v.len() - 1) as f32 * p).round() as usize];
    let (a, b) = (q(0.02), q(0.98));
    if b > a { (a, b) } else { (a, a + 1.0) }
}

/// Couleurs d'un champ : séquentielle entre `bornes`, ou divergente symétrique autour de 0.
pub fn colorer(valeurs: &[f32], divergent: bool) -> (Vec<[u8; 3]>, (f32, f32)) {
    let (a, b) = bornes(valeurs);
    let (bas, haut) = if divergent {
        let m = a.abs().max(b.abs());
        (-m, m)
    } else {
        (a, b)
    };
    let palette = if divergent { divergente } else { sequentielle };
    (
        valeurs.iter().map(|&v| palette((v - bas) / (haut - bas))).collect(),
        (bas, haut),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extremites_et_milieu() {
        assert_eq!(sequentielle(0.0), [68, 1, 84]);
        assert_eq!(sequentielle(1.0), [253, 231, 37]);
        assert_eq!(divergente(0.5), [220, 220, 220]);
        assert_eq!(sequentielle(-3.0), sequentielle(0.0));
        assert_eq!(divergente(f32::NAN), divergente(0.5));
    }

    #[test]
    fn coloration_divergente_symetrique() {
        let (c, (bas, haut)) = colorer(&[-1.0, 0.0, 2.0], true);
        assert_eq!((bas, haut), (-2.0, 2.0));
        assert_eq!(c[1], divergente(0.5));
    }
}
