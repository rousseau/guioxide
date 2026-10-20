//! Table des étiquettes d'une segmentation (`*_dseg.tsv`, format BIDS : `index name abbreviation color`).
//!
//! **Provisoire** pour la lecture du fichier : à remplacer par le lecteur de `medoxide-core`
//! ([medoxide#6](https://github.com/rousseau/medoxide/issues/6)). Les couleurs par défaut relèvent de l'affichage.

use std::collections::BTreeMap;
use std::path::Path;

use crate::{ErreurDonnees, Tableau};

#[derive(Clone, Debug, PartialEq)]
pub struct Etiquette {
    pub nom: String,
    pub abreviation: Option<String>,
    /// Couleur RVB : celle du fichier, sinon [`couleur_par_defaut`].
    pub couleur: [u8; 3],
}

/// Étiquettes par indice ; l'indice 0 est le fond.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableEtiquettes {
    pub etiquettes: BTreeMap<u32, Etiquette>,
}

impl TableEtiquettes {
    pub fn lire(chemin: &Path) -> Result<TableEtiquettes, ErreurDonnees> {
        let t = Tableau::lire(chemin)?;
        if t.colonne("index").is_none() {
            return Err(ErreurDonnees::format(chemin, "colonne « index » absente"));
        }
        let mut etiquettes = BTreeMap::new();
        for i in 0..t.lignes.len() {
            let Some(index) = t.valeur(i, "index") else { continue };
            let index: u32 = index
                .parse()
                .map_err(|_| ErreurDonnees::format(chemin, format!("index invalide : {index}")))?;
            let couleur = t
                .valeur(i, "color")
                .and_then(lire_couleur)
                .unwrap_or_else(|| couleur_par_defaut(index));
            let nom = t.valeur(i, "name").unwrap_or("").to_owned();
            etiquettes.insert(
                index,
                Etiquette {
                    nom,
                    abreviation: t.valeur(i, "abbreviation").map(str::to_owned),
                    couleur,
                },
            );
        }
        Ok(TableEtiquettes { etiquettes })
    }

    /// Couleur d'un indice : celle de la table, sinon la couleur par défaut.
    pub fn couleur(&self, index: u32) -> [u8; 3] {
        self.etiquettes
            .get(&index)
            .map_or_else(|| couleur_par_defaut(index), |e| e.couleur)
    }
}

/// `#rrggbb` → RVB.
fn lire_couleur(texte: &str) -> Option<[u8; 3]> {
    let h = texte.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let canal = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([canal(0)?, canal(2)?, canal(4)?])
}

/// Couleur distincte pour un indice sans couleur déclarée : teintes espacées de l'angle d'or, saturées.
pub fn couleur_par_defaut(index: u32) -> [u8; 3] {
    let teinte = (index as f64 * 137.507_764_05).rem_euclid(360.0) / 60.0;
    let (s, v) = (0.65, 0.95);
    let c = v * s;
    let x = c * (1.0 - (teinte.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match teinte as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|k| ((k + m) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_bids_avec_et_sans_couleur() {
        let dir = std::env::temp_dir().join(format!("guioxide-dseg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let chemin = dir.join("dseg.tsv");
        std::fs::write(
            &chemin,
            "index\tname\tabbreviation\tcolor\n0\tfond\tn/a\tn/a\n1\tcortex\tGM\t#ff8000\n2\tLCS\tCSF\tn/a\n",
        )
        .unwrap();
        let t = TableEtiquettes::lire(&chemin).unwrap();
        assert_eq!(t.etiquettes.len(), 3);
        assert_eq!(t.etiquettes[&1].couleur, [255, 128, 0]);
        assert_eq!(t.etiquettes[&1].abreviation.as_deref(), Some("GM"));
        assert_eq!(t.etiquettes[&0].abreviation, None);
        assert_eq!(t.couleur(2), couleur_par_defaut(2));
        assert_eq!(t.couleur(42), couleur_par_defaut(42));

        std::fs::write(&chemin, "name\nx\n").unwrap();
        assert!(
            TableEtiquettes::lire(&chemin)
                .unwrap_err()
                .to_string()
                .contains("dseg.tsv")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn couleurs_par_defaut_distinctes() {
        let couleurs: Vec<_> = (1..20).map(couleur_par_defaut).collect();
        for (i, a) in couleurs.iter().enumerate() {
            for b in &couleurs[i + 1..] {
                let d: i32 = (0..3).map(|c| (a[c] as i32 - b[c] as i32).abs()).sum();
                assert!(d > 20, "{a:?} trop proche de {b:?}");
            }
        }
    }
}
