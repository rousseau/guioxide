//! Validation par l'opérateur : `derivatives/guioxide/sub-X/ses-Y/sub-X_ses-Y_desc-qc.json` (`docs/donnees.md`).
//!
//! Seule écriture de guioxide : ses propres annotations, jamais dans les dérivées des outils.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::{ErreurBids, Etape};

/// Avis de l'opérateur sur une étape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Avis {
    Valide,
    ARevoir,
    Rejete,
}

impl Avis {
    pub const TOUS: [Avis; 3] = [Avis::Valide, Avis::ARevoir, Avis::Rejete];

    /// Valeur dans le fichier.
    pub fn code(self) -> &'static str {
        match self {
            Avis::Valide => "valide",
            Avis::ARevoir => "a_revoir",
            Avis::Rejete => "rejete",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Avis::Valide => "validé",
            Avis::ARevoir => "à revoir",
            Avis::Rejete => "rejeté",
        }
    }

    fn depuis_code(code: &str) -> Option<Avis> {
        Avis::TOUS.into_iter().find(|a| a.code() == code)
    }
}

/// Évaluation d'une étape.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    pub avis: Avis,
    pub note: String,
    pub operateur: String,
    /// Date ISO 8601 (UTC).
    pub date: String,
    /// Outil et version de la dérivée évaluée (`GeneratedBy`), si connus.
    pub outil: Option<String>,
}

/// Évaluations d'une session, par étape ; une étape absente n'est pas encore évaluée.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Qc {
    pub etapes: BTreeMap<String, Evaluation>,
}

/// Clé d'une étape dans le fichier.
pub fn cle(etape: Etape) -> &'static str {
    match etape {
        Etape::Examen => "examen",
        Etape::Reconstruction => "reconstruction",
        Etape::Surfaces => "surfaces",
        Etape::Simulation => "simulation",
    }
}

impl Qc {
    pub fn evaluation(&self, etape: Etape) -> Option<&Evaluation> {
        self.etapes.get(cle(etape))
    }

    /// Lit le fichier ; absent → aucune évaluation.
    pub fn lire(chemin: &Path) -> Result<Qc, ErreurBids> {
        if !chemin.exists() {
            return Ok(Qc::default());
        }
        let format = |m: String| ErreurBids::Format {
            chemin: chemin.to_path_buf(),
            message: m,
        };
        let texte = std::fs::read_to_string(chemin).map_err(|e| ErreurBids::lecture(chemin, e))?;
        let v: Value = serde_json::from_str(&texte).map_err(|e| format(e.to_string()))?;
        let objet = v.as_object().ok_or_else(|| format("objet JSON attendu".into()))?;
        let mut etapes = BTreeMap::new();
        for (etape, e) in objet {
            let texte_de = |c: &str| e[c].as_str().unwrap_or_default().to_owned();
            let avis = Avis::depuis_code(&texte_de("etat"))
                .ok_or_else(|| format(format!("{etape} : état « {} » inconnu", texte_de("etat"))))?;
            let outil = e["outil"].as_str().map(str::to_owned);
            etapes.insert(
                etape.clone(),
                Evaluation {
                    avis,
                    note: texte_de("note"),
                    operateur: texte_de("operateur"),
                    date: texte_de("date"),
                    outil,
                },
            );
        }
        Ok(Qc { etapes })
    }

    /// Écrit le fichier de façon atomique (fichier temporaire puis renommage), et le `dataset_description.json` de
    /// `derivatives/guioxide/` s'il manque. `racine_guioxide` : le dossier `derivatives/guioxide`.
    pub fn ecrire(&self, chemin: &Path, racine_guioxide: &Path) -> Result<(), ErreurBids> {
        let ecrire = |c: &Path, texte: String| -> Result<(), ErreurBids> {
            let dossier = c.parent().unwrap_or(Path::new("."));
            std::fs::create_dir_all(dossier).map_err(|e| ErreurBids::lecture(dossier, e))?;
            let temporaire = c.with_extension("json.tmp");
            std::fs::write(&temporaire, texte).map_err(|e| ErreurBids::lecture(&temporaire, e))?;
            std::fs::rename(&temporaire, c).map_err(|e| ErreurBids::lecture(c, e))
        };
        let description = racine_guioxide.join("dataset_description.json");
        if !description.exists() {
            let d = json!({
                "Name": "guioxide : validation par l'opérateur",
                "BIDSVersion": "1.9.0",
                "DatasetType": "derivative",
                "GeneratedBy": [{"Name": "guioxide", "Version": env!("CARGO_PKG_VERSION")}],
            });
            ecrire(&description, format!("{:#}\n", d))?;
        }
        let mut objet = serde_json::Map::new();
        for (etape, e) in &self.etapes {
            let mut v = json!({"etat": e.avis.code(), "note": e.note, "operateur": e.operateur, "date": e.date});
            if let Some(o) = &e.outil {
                v["outil"] = json!(o);
            }
            objet.insert(etape.clone(), v);
        }
        ecrire(chemin, format!("{:#}\n", Value::Object(objet)))
    }
}

/// Date et heure UTC actuelles, ISO 8601 à la seconde.
pub fn maintenant_iso() -> String {
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso_depuis_secondes(s)
}

/// Secondes depuis l'époque → `AAAA-MM-JJTHH:MM:SSZ` (jours civils, algorithme de H. Hinnant).
pub fn iso_depuis_secondes(s: i64) -> String {
    let (jours, reste) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let jde = z - ere * 146_097;
    let ade = (jde - jde / 1460 + jde / 36_524 - jde / 146_096) / 365;
    let jda = jde - (365 * ade + ade / 4 - ade / 100);
    let mp = (5 * jda + 2) / 153;
    let jour = jda - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let an = ade + ere * 400 + i64::from(mois <= 2);
    format!(
        "{an:04}-{mois:02}-{jour:02}T{:02}:{:02}:{:02}Z",
        reste / 3600,
        reste / 60 % 60,
        reste % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_iso() {
        assert_eq!(iso_depuis_secondes(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_depuis_secondes(951_868_800), "2000-03-01T00:00:00Z");
        assert_eq!(iso_depuis_secondes(1_792_405_815), "2026-10-19T10:30:15Z");
        assert_eq!(iso_depuis_secondes(951_868_799), "2000-02-29T23:59:59Z");
    }

    #[test]
    fn aller_retour_et_description_du_derive() {
        let dir = tempfile::tempdir().unwrap();
        let racine = dir.path().join("derivatives/guioxide");
        let chemin = racine.join("sub-01/ses-01/sub-01_ses-01_desc-qc.json");
        assert_eq!(Qc::lire(&chemin).unwrap(), Qc::default(), "absent : rien d'évalué");

        let mut qc = Qc::default();
        qc.etapes.insert(
            cle(Etape::Examen).into(),
            Evaluation {
                avis: Avis::ARevoir,
                note: "pile coronale 2 bouge".into(),
                operateur: "FR".into(),
                date: "2026-10-20T10:12:00Z".into(),
                outil: Some("medx 0.1.0".into()),
            },
        );
        qc.ecrire(&chemin, &racine).unwrap();
        assert_eq!(Qc::lire(&chemin).unwrap(), qc);
        assert!(
            !chemin.with_extension("json.tmp").exists(),
            "pas de fichier temporaire laissé"
        );
        let d: Value =
            serde_json::from_str(&std::fs::read_to_string(racine.join("dataset_description.json")).unwrap()).unwrap();
        assert_eq!(
            (d["DatasetType"].as_str(), d["GeneratedBy"][0]["Name"].as_str()),
            (Some("derivative"), Some("guioxide"))
        );
        assert_eq!(qc.evaluation(Etape::Examen).map(|e| e.avis), Some(Avis::ARevoir));
        assert_eq!(qc.evaluation(Etape::Surfaces), None);

        std::fs::write(&chemin, r#"{"examen": {"etat": "peut-etre"}}"#).unwrap();
        assert!(Qc::lire(&chemin).unwrap_err().to_string().contains("peut-etre"));
    }
}
