//! Runs de gyroxide (`docs/donnees.md`, étape 4) : `status.json`, `sequence.tsv`, dossiers `run-<label>`.

use std::path::{Path, PathBuf};

use crate::{ErreurDonnees, Tableau};

/// Contenu de `status.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct EtatRun {
    /// `running`, `done` ou `failed`.
    pub state: String,
    pub step: Option<u64>,
    pub total_steps: Option<u64>,
    /// Dates ISO 8601.
    pub started: Option<String>,
    pub updated: Option<String>,
    pub message: Option<String>,
}

impl EtatRun {
    /// Avancement dans [0, 1], si le nombre total de pas est connu.
    pub fn avancement(&self) -> Option<f64> {
        let (s, t) = (self.step?, self.total_steps?);
        (t > 0).then(|| (s as f64 / t as f64).clamp(0.0, 1.0))
    }
}

/// Une ligne de `sequence.tsv` : une surface écrite au pas `step`.
#[derive(Clone, Debug, PartialEq)]
pub struct PasSequence {
    pub step: u64,
    /// Temps du modèle.
    pub time: Option<f64>,
    /// Âge gestationnel (semaines), si le modèle définit la correspondance.
    pub ga_weeks: Option<f64>,
    /// Nom du fichier de surface, relatif au dossier du run.
    pub file: String,
    pub gi: Option<f64>,
    pub energy: Option<f64>,
    pub n_contacts: Option<u64>,
}

pub fn lire_etat_run(chemin: &Path) -> Result<EtatRun, ErreurDonnees> {
    let texte = std::fs::read_to_string(chemin).map_err(|e| ErreurDonnees::lecture(chemin, e))?;
    let v: serde_json::Value = serde_json::from_str(&texte).map_err(|e| ErreurDonnees::format(chemin, e))?;
    let texte_de = |cle: &str| v[cle].as_str().map(str::to_owned);
    Ok(EtatRun {
        state: texte_de("state").ok_or_else(|| ErreurDonnees::format(chemin, "champ « state » absent"))?,
        step: v["step"].as_u64(),
        total_steps: v["total_steps"].as_u64(),
        started: texte_de("started"),
        updated: texte_de("updated"),
        message: texte_de("message"),
    })
}

/// Lit `sequence.tsv` ; colonnes obligatoires `step` et `file`. Les lignes sont triées par pas.
pub fn lire_sequence(chemin: &Path) -> Result<Vec<PasSequence>, ErreurDonnees> {
    let t = Tableau::lire(chemin)?;
    let mut pas = Vec::with_capacity(t.lignes.len());
    for i in 0..t.lignes.len() {
        let manque =
            |c: &str| ErreurDonnees::format(chemin, format!("ligne {} : colonne « {c} » vide ou absente", i + 2));
        let nombre = |c: &str| -> Result<Option<f64>, ErreurDonnees> {
            t.valeur(i, c)
                .map(|v| {
                    v.parse()
                        .map_err(|_| ErreurDonnees::format(chemin, format!("ligne {} : {c} = « {v} »", i + 2)))
                })
                .transpose()
        };
        pas.push(PasSequence {
            step: nombre("step")?.ok_or_else(|| manque("step"))? as u64,
            time: nombre("time")?,
            ga_weeks: nombre("ga_weeks")?,
            file: t.valeur(i, "file").ok_or_else(|| manque("file"))?.to_owned(),
            gi: nombre("gi")?,
            energy: nombre("energy")?,
            n_contacts: nombre("n_contacts")?.map(|n| n as u64),
        });
    }
    pas.sort_by_key(|p| p.step);
    Ok(pas)
}

/// Dossiers `run-<label>` d'un dossier de simulations, triés : `(label, chemin)`.
pub fn lister_runs(dossier: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return Vec::new();
    };
    let mut runs: Vec<(String, PathBuf)> = entrees
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter_map(|p| Some((p.file_name()?.to_str()?.strip_prefix("run-")?.to_owned(), p.clone())))
        .collect();
    runs.sort();
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etat_et_sequence_d_un_run() {
        let dir = tempfile::tempdir().unwrap();
        let run = dir.path().join("run-essai");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::create_dir_all(dir.path().join("pas-un-run")).unwrap();
        std::fs::write(
            run.join("status.json"),
            r#"{"state": "running", "step": 250, "total_steps": 1000, "started": "2026-10-19T10:00:00Z"}"#,
        )
        .unwrap();
        std::fs::write(
            run.join("sequence.tsv"),
            "step\ttime\tga_weeks\tfile\tgi\tenergy\tn_contacts\n\
             500\t0.5\tn/a\tb.surf.gii\t1.2\t0.01\t3\n\
             0\t0\t21\ta.surf.gii\t1.0\tn/a\t0\n",
        )
        .unwrap();
        assert_eq!(lister_runs(dir.path()), [("essai".to_owned(), run.clone())]);
        let e = lire_etat_run(&run.join("status.json")).unwrap();
        assert_eq!(
            (e.state.as_str(), e.avancement(), e.message),
            ("running", Some(0.25), None)
        );
        let s = lire_sequence(&run.join("sequence.tsv")).unwrap();
        assert_eq!(s.iter().map(|p| p.step).collect::<Vec<_>>(), [0, 500], "triée par pas");
        assert_eq!(
            (s[0].ga_weeks, s[0].energy, s[1].n_contacts),
            (Some(21.0), None, Some(3))
        );

        std::fs::write(run.join("status.json"), r#"{"step": 3}"#).unwrap();
        assert!(
            lire_etat_run(&run.join("status.json"))
                .unwrap_err()
                .to_string()
                .contains("state")
        );
        std::fs::write(run.join("sequence.tsv"), "step\tfile\nx\ta.gii\n").unwrap();
        assert!(
            lire_sequence(&run.join("sequence.tsv"))
                .unwrap_err()
                .to_string()
                .contains("ligne 2")
        );
    }
}
