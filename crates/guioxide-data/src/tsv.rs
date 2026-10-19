//! Lecture des tableaux TSV : ligne d'en-tête, séparateur tabulation, `n/a` pour une valeur absente.

use std::path::Path;

use crate::ErreurDonnees;

/// Un tableau TSV lu en entier (les fichiers BIDS lus ici sont petits).
#[derive(Clone, Debug, Default)]
pub struct Tableau {
    pub colonnes: Vec<String>,
    pub lignes: Vec<Vec<String>>,
}

impl Tableau {
    pub fn lire(chemin: &Path) -> Result<Tableau, ErreurDonnees> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| ErreurDonnees::lecture(chemin, e))?;
        Ok(Tableau::depuis_texte(&texte))
    }

    pub fn depuis_texte(texte: &str) -> Tableau {
        let mut lignes = texte.lines().filter(|l| !l.trim().is_empty());
        let decouper = |l: &str| l.split('\t').map(|c| c.trim().to_owned()).collect::<Vec<_>>();
        let colonnes = lignes.next().map(decouper).unwrap_or_default();
        Tableau {
            colonnes,
            lignes: lignes.map(decouper).collect(),
        }
    }

    pub fn colonne(&self, nom: &str) -> Option<usize> {
        self.colonnes.iter().position(|c| c == nom)
    }

    /// Valeur de la colonne `nom` sur la ligne `i` ; `None` si la colonne manque, ou si la valeur est vide ou `n/a`.
    pub fn valeur(&self, i: usize, nom: &str) -> Option<&str> {
        let v = self.lignes.get(i)?.get(self.colonne(nom)?)?.as_str();
        (!v.is_empty() && v != "n/a").then_some(v)
    }

    /// Première ligne dont la colonne `cle` vaut `valeur`.
    pub fn ligne_ou(&self, cle: &str, valeur: &str) -> Option<usize> {
        (0..self.lignes.len()).find(|&i| self.valeur(i, cle) == Some(valeur))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valeurs_absentes_et_colonnes_manquantes() {
        let t = Tableau::depuis_texte("session_id\tga_weeks\nses-01\t31.5\nses-02\tn/a\nses-03\t\n\n");
        assert_eq!(t.lignes.len(), 3);
        assert_eq!(t.valeur(0, "ga_weeks"), Some("31.5"));
        assert_eq!(t.valeur(1, "ga_weeks"), None);
        assert_eq!(t.valeur(2, "ga_weeks"), None);
        assert_eq!(t.valeur(0, "absente"), None);
        assert_eq!(t.ligne_ou("session_id", "ses-02"), Some(1));
        assert_eq!(t.ligne_ou("session_id", "ses-09"), None);
    }

    #[test]
    fn fin_de_ligne_windows() {
        let t = Tableau::depuis_texte("participant_id\tga_weeks\r\nsub-01\t30\r\n");
        assert_eq!(t.valeur(0, "ga_weeks"), Some("30"));
    }
}
