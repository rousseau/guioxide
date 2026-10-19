//! Indexation d'un jeu BIDS : racine, sujets, sessions, piles, âge gestationnel, dérivées.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{ErreurBids, Tableau};

/// Colonne de l'âge gestationnel (semaines d'aménorrhée), dans `sub-*_sessions.tsv` ou `participants.tsv`.
pub const COLONNE_AGE: &str = "ga_weeks";

/// Un jeu BIDS indexé.
#[derive(Clone, Debug)]
pub struct JeuBids {
    /// Racine BIDS : le dossier qui contient `dataset_description.json` et les `sub-*`.
    pub racine: PathBuf,
    /// Racine de l'étude, qui contient `derivatives/` : le parent de la racine BIDS quand celle-ci s'appelle
    /// `sourcedata` (convention des buckets Garage), sinon la racine BIDS elle-même.
    pub racine_etude: PathBuf,
    pub nom: String,
    pub version_bids: Option<String>,
    pub sujets: Vec<Sujet>,
    /// Dérivées de l'étude (`<racine_etude>/derivatives/*`).
    pub derivees: Vec<Derivee>,
    /// Incohérences relevées, qui n'empêchent pas l'ouverture.
    pub avertissements: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Sujet {
    /// Identifiant complet, `sub-…`.
    pub id: String,
    pub sessions: Vec<Session>,
}

#[derive(Clone, Debug)]
pub struct Session {
    /// Identifiant complet, `ses-…` ; `None` si le sujet n'a pas de niveau session.
    pub id: Option<String>,
    /// Âge gestationnel à l'examen, en semaines.
    pub age_gestationnel: Option<f64>,
    /// Images de `anat/`, triées par nom.
    pub piles: Vec<Pile>,
}

/// Une image acquise (une pile de coupes en IRM fœtale).
#[derive(Clone, Debug)]
pub struct Pile {
    pub chemin: PathBuf,
    /// Nom du fichier sans extension : c'est ainsi que le contrat désigne une pile.
    pub nom: String,
    /// Suffixe BIDS : `T2w`, `T1w`, …
    pub contraste: String,
}

/// Un dossier `derivatives/<pipeline>/`.
#[derive(Clone, Debug)]
pub struct Derivee {
    pub pipeline: String,
    pub chemin: PathBuf,
    /// Outil et version, lus dans `GeneratedBy` de son `dataset_description.json`.
    pub generateur: Option<String>,
}

impl Session {
    /// Préfixe des noms de fichiers : `sub-X_ses-Y`, ou `sub-X` sans session.
    pub fn prefixe(&self, sujet: &Sujet) -> String {
        match &self.id {
            Some(ses) => format!("{}_{ses}", sujet.id),
            None => sujet.id.clone(),
        }
    }

    /// Dossier relatif de la session : `sub-X/ses-Y`, ou `sub-X`.
    pub fn dossier(&self, sujet: &Sujet) -> PathBuf {
        let mut d = PathBuf::from(&sujet.id);
        if let Some(ses) = &self.id {
            d.push(ses);
        }
        d
    }

    pub fn contrastes(&self) -> BTreeSet<&str> {
        self.piles.iter().map(|p| p.contraste.as_str()).collect()
    }
}

impl JeuBids {
    /// Dossier des dérivées de l'étude.
    pub fn derivees_etude(&self) -> PathBuf {
        self.racine_etude.join("derivatives")
    }

    pub fn nb_sessions(&self) -> usize {
        self.sujets.iter().map(|s| s.sessions.len()).sum()
    }

    /// Contrastes présents dans le jeu.
    pub fn contrastes(&self) -> BTreeSet<&str> {
        self.sujets
            .iter()
            .flat_map(|s| &s.sessions)
            .flat_map(|s| s.contrastes())
            .collect()
    }
}

/// Trouve la racine BIDS à partir d'un dossier déposé : la racine elle-même, la racine d'un bucket (qui contient
/// `sourcedata/`), ou n'importe quel dossier en dessous (`sub-*`, `ses-*`, `anat/`, un fichier).
///
/// On remonte jusqu'au premier dossier qui contient un `dataset_description.json` de jeu brut ; ceux des dérivées
/// (`DatasetType: derivative`) sont ignorés.
pub fn trouver_racine(chemin: &Path) -> Result<PathBuf, ErreurBids> {
    let chemin = chemin.canonicalize().map_err(|e| ErreurBids::lecture(chemin, e))?;
    let source = chemin.join("sourcedata");
    if !chemin.join("dataset_description.json").is_file() && source.join("dataset_description.json").is_file() {
        return Ok(source);
    }
    for dossier in chemin.ancestors().filter(|d| d.is_dir()) {
        let description = dossier.join("dataset_description.json");
        if description.is_file() && !est_derivee(&description) {
            return Ok(dossier.to_path_buf());
        }
    }
    Err(ErreurBids::RacineIntrouvable(chemin))
}

fn est_derivee(description: &Path) -> bool {
    lire_json(description).is_ok_and(|v| v["DatasetType"].as_str() == Some("derivative"))
}

fn lire_json(chemin: &Path) -> Result<serde_json::Value, ErreurBids> {
    let texte = std::fs::read_to_string(chemin).map_err(|e| ErreurBids::lecture(chemin, e))?;
    serde_json::from_str(&texte).map_err(|e| ErreurBids::Format {
        chemin: chemin.to_path_buf(),
        message: e.to_string(),
    })
}

/// Indexe le jeu BIDS qui contient `chemin` (voir [`trouver_racine`]).
pub fn indexer(chemin: &Path) -> Result<JeuBids, ErreurBids> {
    let racine = trouver_racine(chemin)?;
    let description = lire_json(&racine.join("dataset_description.json"))?;
    let racine_etude = match racine.file_name() {
        Some(n) if n == "sourcedata" => racine.parent().unwrap_or(&racine).to_path_buf(),
        _ => racine.clone(),
    };
    let nom = description["Name"].as_str().map(str::to_owned).unwrap_or_else(|| {
        racine_etude
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    let mut avertissements = Vec::new();

    let participants = racine.join("participants.tsv");
    let participants = if participants.is_file() {
        Some(Tableau::lire(&participants)?)
    } else {
        avertissements.push("participants.tsv absent".to_owned());
        None
    };

    let mut sujets = Vec::new();
    for dossier in sous_dossiers(&racine, "sub-")? {
        sujets.push(indexer_sujet(&dossier, participants.as_ref(), &mut avertissements)?);
    }
    if let Some(p) = &participants {
        let presents: BTreeSet<&str> = sujets.iter().map(|s| s.id.as_str()).collect();
        for i in 0..p.lignes.len() {
            if let Some(id) = p.valeur(i, "participant_id")
                && !presents.contains(id)
            {
                avertissements.push(format!("{id} : listé dans participants.tsv, sans dossier"));
            }
        }
        for s in &sujets {
            if p.ligne_ou("participant_id", &s.id).is_none() {
                avertissements.push(format!("{} : absent de participants.tsv", s.id));
            }
        }
    }

    let derivees = lister_derivees(&racine_etude.join("derivatives"))?;
    Ok(JeuBids {
        racine,
        racine_etude,
        nom,
        version_bids: description["BIDSVersion"].as_str().map(str::to_owned),
        sujets,
        derivees,
        avertissements,
    })
}

fn indexer_sujet(
    dossier: &Path,
    participants: Option<&Tableau>,
    avertissements: &mut Vec<String>,
) -> Result<Sujet, ErreurBids> {
    let id = dossier.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let tsv_sessions = dossier.join(format!("{id}_sessions.tsv"));
    let ages_sessions = if tsv_sessions.is_file() {
        Some(Tableau::lire(&tsv_sessions)?)
    } else {
        None
    };
    let age_participant = participants
        .and_then(|p| p.ligne_ou("participant_id", &id).and_then(|i| p.valeur(i, COLONNE_AGE)))
        .and_then(|v| v.parse().ok());

    let mut sessions = Vec::new();
    for dossier_ses in sous_dossiers(dossier, "ses-")? {
        let ses = dossier_ses
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let age = ages_sessions
            .as_ref()
            .and_then(|t| t.ligne_ou("session_id", &ses).and_then(|i| t.valeur(i, COLONNE_AGE)))
            .and_then(|v| v.parse().ok());
        sessions.push(Session {
            id: Some(ses),
            age_gestationnel: age,
            piles: lister_piles(&dossier_ses.join("anat"))?,
        });
    }
    if sessions.is_empty() {
        let piles = lister_piles(&dossier.join("anat"))?;
        if piles.is_empty() {
            avertissements.push(format!("{id} : ni session ni image anat"));
        }
        sessions.push(Session {
            id: None,
            age_gestationnel: age_participant,
            piles,
        });
    }
    Ok(Sujet { id, sessions })
}

/// Images NIfTI d'un dossier `anat/` ; dossier absent → aucune.
fn lister_piles(anat: &Path) -> Result<Vec<Pile>, ErreurBids> {
    if !anat.is_dir() {
        return Ok(Vec::new());
    }
    let mut piles = Vec::new();
    for entree in std::fs::read_dir(anat).map_err(|e| ErreurBids::lecture(anat, e))? {
        let chemin = entree.map_err(|e| ErreurBids::lecture(anat, e))?.path();
        let fichier = chemin.file_name().unwrap_or_default().to_string_lossy();
        let Some(nom) = fichier.strip_suffix(".nii.gz").or_else(|| fichier.strip_suffix(".nii")) else {
            continue;
        };
        let contraste = nom.rsplit('_').next().unwrap_or(nom).to_owned();
        piles.push(Pile {
            nom: nom.to_owned(),
            contraste,
            chemin: chemin.clone(),
        });
    }
    piles.sort_by(|a, b| a.nom.cmp(&b.nom));
    Ok(piles)
}

/// Dossiers `derivatives/<pipeline>/`, avec l'outil et la version déclarés ; dossier absent → aucun.
pub fn lister_derivees(derivatives: &Path) -> Result<Vec<Derivee>, ErreurBids> {
    if !derivatives.is_dir() {
        return Ok(Vec::new());
    }
    let mut derivees = Vec::new();
    for chemin in sous_dossiers(derivatives, "")? {
        let generateur = lire_json(&chemin.join("dataset_description.json")).ok().and_then(|d| {
            let g = d["GeneratedBy"].get(0)?;
            let nom = g["Name"].as_str()?;
            Some(match g["Version"].as_str() {
                Some(v) => format!("{nom} {v}"),
                None => nom.to_owned(),
            })
        });
        let pipeline = chemin.file_name().unwrap_or_default().to_string_lossy().into_owned();
        derivees.push(Derivee {
            pipeline,
            chemin,
            generateur,
        });
    }
    Ok(derivees)
}

/// Sous-dossiers dont le nom commence par `prefixe` (dossiers cachés exclus), triés par nom.
fn sous_dossiers(dossier: &Path, prefixe: &str) -> Result<Vec<PathBuf>, ErreurBids> {
    let mut noms = BTreeMap::new();
    for entree in std::fs::read_dir(dossier).map_err(|e| ErreurBids::lecture(dossier, e))? {
        let chemin = entree.map_err(|e| ErreurBids::lecture(dossier, e))?.path();
        let nom = chemin.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if chemin.is_dir() && nom.starts_with(prefixe) && !nom.starts_with('.') {
            noms.insert(nom, chemin);
        }
    }
    Ok(noms.into_values().collect())
}
