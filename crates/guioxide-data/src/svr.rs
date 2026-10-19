//! Sorties du SVR lues par guioxide (`docs/donnees.md`, étapes 1 et 2) : qualité et mouvement par coupe.

use std::path::Path;

use guioxide_geom::Affine;

use crate::{ErreurDonnees, Tableau};

/// Une ligne de `*_slicequality.tsv`.
#[derive(Clone, Debug, PartialEq)]
pub struct QualiteCoupe {
    /// Nom du fichier de la pile, sans extension.
    pub stack: String,
    pub slice: usize,
    pub registered: bool,
    pub score_before: Option<f64>,
    pub score_after: Option<f64>,
    pub n_registrations: Option<u32>,
    pub correction_rms_mm: Option<f64>,
    pub excluded: bool,
    pub reason: Option<String>,
}

/// Une ligne de `*_motion.tsv` : mouvement rigide de la coupe, dans le repère monde.
#[derive(Clone, Debug, PartialEq)]
pub struct MouvementCoupe {
    pub stack: String,
    pub slice: usize,
    pub matrice: Affine,
}

fn obligatoire<'a>(t: &'a Tableau, i: usize, colonne: &str, chemin: &Path) -> Result<&'a str, ErreurDonnees> {
    t.valeur(i, colonne).ok_or_else(|| {
        ErreurDonnees::format(
            chemin,
            format!("ligne {} : colonne « {colonne} » vide ou absente", i + 2),
        )
    })
}

fn nombre<T: std::str::FromStr>(texte: &str, colonne: &str, i: usize, chemin: &Path) -> Result<T, ErreurDonnees> {
    texte
        .parse()
        .map_err(|_| ErreurDonnees::format(chemin, format!("ligne {} : « {texte} » invalide pour {colonne}", i + 2)))
}

/// Lit `*_slicequality.tsv`. Colonnes obligatoires : `stack`, `slice` ; les autres peuvent manquer (valeur absente,
/// coupe ni recalée ni exclue).
pub fn lire_qualite(chemin: &Path) -> Result<Vec<QualiteCoupe>, ErreurDonnees> {
    let t = Tableau::lire(chemin)?;
    let mut lignes = Vec::with_capacity(t.lignes.len());
    for i in 0..t.lignes.len() {
        let option = |colonne: &str| {
            t.valeur(i, colonne)
                .map(|v| nombre::<f64>(v, colonne, i, chemin))
                .transpose()
        };
        let drapeau = |colonne: &str| t.valeur(i, colonne).is_some_and(|v| v == "1");
        lignes.push(QualiteCoupe {
            stack: obligatoire(&t, i, "stack", chemin)?.to_owned(),
            slice: nombre(obligatoire(&t, i, "slice", chemin)?, "slice", i, chemin)?,
            registered: drapeau("registered"),
            score_before: option("score_before")?,
            score_after: option("score_after")?,
            n_registrations: t
                .valeur(i, "n_registrations")
                .map(|v| nombre(v, "n_registrations", i, chemin))
                .transpose()?,
            correction_rms_mm: option("correction_rms_mm")?,
            excluded: drapeau("excluded"),
            reason: t.valeur(i, "reason").map(str::to_owned),
        });
    }
    Ok(lignes)
}

/// Lit `*_motion.tsv` : `stack`, `slice`, puis la matrice 4 × 4 en colonnes `m<ligne><colonne>`.
pub fn lire_mouvement(chemin: &Path) -> Result<Vec<MouvementCoupe>, ErreurDonnees> {
    let t = Tableau::lire(chemin)?;
    let mut lignes = Vec::with_capacity(t.lignes.len());
    for i in 0..t.lignes.len() {
        let mut m = [[0.0; 4]; 4];
        for (r, ligne) in m.iter_mut().enumerate() {
            for (c, x) in ligne.iter_mut().enumerate() {
                let colonne = format!("m{r}{c}");
                *x = nombre(obligatoire(&t, i, &colonne, chemin)?, &colonne, i, chemin)?;
            }
        }
        lignes.push(MouvementCoupe {
            stack: obligatoire(&t, i, "stack", chemin)?.to_owned(),
            slice: nombre(obligatoire(&t, i, "slice", chemin)?, "slice", i, chemin)?,
            matrice: Affine(m),
        });
    }
    Ok(lignes)
}

/// Mouvement d'une coupe, résumé pour la courbe de mouvement.
#[derive(Clone, Debug, PartialEq)]
pub struct PointMouvement {
    pub stack: String,
    pub slice: usize,
    /// Déplacement du centre de la coupe (mm).
    pub translation: f64,
    /// Angle de la rotation (degrés).
    pub rotation: f64,
    pub exclue: bool,
}

/// Mouvement par coupe : déplacement du centre de chaque coupe (`‖M·c − c‖`, `c` le centre de la coupe dans sa
/// géométrie d'en-tête) et angle de rotation de `M`. `geometrie` donne les dimensions et l'affine d'une pile par son
/// nom ; les coupes d'une pile sans géométrie sont omises. Les coupes exclues sont lues dans `qualite`.
pub fn mouvement_par_coupe(
    mouvement: &[MouvementCoupe],
    geometrie: impl Fn(&str) -> Option<([usize; 3], Affine)>,
    qualite: Option<&[QualiteCoupe]>,
) -> Vec<PointMouvement> {
    let exclues: std::collections::HashSet<(&str, usize)> = qualite
        .unwrap_or_default()
        .iter()
        .filter(|l| l.excluded)
        .map(|l| (l.stack.as_str(), l.slice))
        .collect();
    mouvement
        .iter()
        .filter_map(|l| {
            let (dims, affine) = geometrie(&l.stack)?;
            let centre = affine.appliquer([
                (dims[0] as f64 - 1.0) / 2.0,
                (dims[1] as f64 - 1.0) / 2.0,
                l.slice as f64,
            ]);
            Some(PointMouvement {
                stack: l.stack.clone(),
                slice: l.slice,
                translation: l.matrice.deplacement(centre),
                rotation: l.matrice.angle_rotation(),
                exclue: exclues.contains(&(l.stack.as_str(), l.slice)),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ecrire(nom: &str, texte: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let chemin = dir.path().join(nom);
        std::fs::write(&chemin, texte).unwrap();
        (dir, chemin)
    }

    #[test]
    fn qualite_selon_le_contrat() {
        let (_d, c) = ecrire(
            "q.tsv",
            "stack\tslice\tregistered\tscore_before\tscore_after\tn_registrations\tcorrection_rms_mm\texcluded\treason\n\
             p1\t0\t1\t0.5\t0.8\t2\t1.25\t0\tn/a\n\
             p1\t1\t0\tn/a\tn/a\t0\t0\t1\toutlier\n",
        );
        let q = lire_qualite(&c).unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(
            (q[0].registered, q[0].score_after, q[0].excluded, q[0].reason.as_deref()),
            (true, Some(0.8), false, None)
        );
        assert_eq!(
            (q[1].score_before, q[1].excluded, q[1].reason.as_deref()),
            (None, true, Some("outlier"))
        );
    }

    #[test]
    fn qualite_invalide_avec_le_numero_de_ligne() {
        let (_d, c) = ecrire("q.tsv", "stack\tslice\tscore_after\np1\t0\t0.5\np1\tdeux\t0.5\n");
        let e = lire_qualite(&c).unwrap_err().to_string();
        assert!(
            e.contains("q.tsv") && e.contains("ligne 3") && e.contains("deux"),
            "{e}"
        );
    }

    #[test]
    fn mouvement_en_ordre_colonne() {
        let mut entete = vec!["stack".to_owned(), "slice".to_owned()];
        let mut valeurs = vec!["p1".to_owned(), "3".to_owned()];
        // Translation (5, 6, 7) : m03, m13, m23.
        for c in 0..4 {
            for r in 0..4 {
                entete.push(format!("m{r}{c}"));
                let v = match (r, c) {
                    (0, 3) => 5.0,
                    (1, 3) => 6.0,
                    (2, 3) => 7.0,
                    (r, c) if r == c => 1.0,
                    _ => 0.0,
                };
                valeurs.push(v.to_string());
            }
        }
        let (_d, chemin) = ecrire("m.tsv", &format!("{}\n{}\n", entete.join("\t"), valeurs.join("\t")));
        let m = lire_mouvement(&chemin).unwrap();
        assert_eq!((m[0].stack.as_str(), m[0].slice), ("p1", 3));
        assert_eq!(m[0].matrice.appliquer([0.0, 0.0, 0.0]), [5.0, 6.0, 7.0]);
    }
}
