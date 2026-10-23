//! État d'avancement d'une session, déduit des fichiers du contrat (`docs/donnees.md`, dernière section).

use std::io::Read;
use std::path::{Path, PathBuf};

use guioxide_data::{State, lire_etat_run, lister_runs};

use crate::CheminsSession;
use crate::jeu::{Session, Sujet};
use crate::{Etape, EtatEtape};

/// États des quatre étapes d'une session, lus dans `derivees` (le dossier `derivatives/` de la sortie choisie).
pub fn etats_session(derivees: &Path, sujet: &Sujet, session: &Session) -> [EtatEtape; 4] {
    Etape::TOUTES.map(|e| etat_etape(e, derivees, sujet, session))
}

pub fn etat_etape(etape: Etape, derivees: &Path, sujet: &Sujet, session: &Session) -> EtatEtape {
    let chemins = CheminsSession::new(derivees, sujet, session);
    match etape {
        Etape::Examen => {
            let masques: Vec<PathBuf> = session
                .piles
                .iter()
                .filter(|p| p.contraste == "T2w")
                .map(|p| chemins.masque(p))
                .collect();
            selon_fichiers(&masques)
        }
        Etape::Reconstruction => selon_fichiers(&[chemins.svr(), chemins.segmentation()]),
        Etape::Surfaces => {
            let surfaces = |hemi: Option<&str>| vec![chemins.surface("white", hemi), chemins.surface("pial", hemi)];
            let par_hemisphere = [surfaces(Some("L")), surfaces(Some("R"))].concat();
            // Deux hémisphères séparés, ou une surface qui couvre les deux (sans `hemi-`) : le plus avancé l'emporte.
            match (selon_fichiers(&par_hemisphere), selon_fichiers(&surfaces(None))) {
                (e @ EtatEtape::Erreur(_), _) | (_, e @ EtatEtape::Erreur(_)) => e,
                (EtatEtape::Terminee, _) | (_, EtatEtape::Terminee) => EtatEtape::Terminee,
                (EtatEtape::EnCours, _) | (_, EtatEtape::EnCours) => EtatEtape::EnCours,
                _ => EtatEtape::NonFaite,
            }
        }
        Etape::Simulation => etat_simulation(&chemins.simulations()),
    }
}

/// Aucun des fichiers → non faite ; tous → terminée ; une partie → en cours ; un fichier illisible → erreur.
fn selon_fichiers(fichiers: &[PathBuf]) -> EtatEtape {
    let mut presents = 0;
    for f in fichiers {
        match verifier(f) {
            None => {}
            Some(Ok(())) => presents += 1,
            Some(Err(message)) => return EtatEtape::Erreur(message),
        }
    }
    match presents {
        0 => EtatEtape::NonFaite,
        n if n == fichiers.len() => EtatEtape::Terminee,
        _ => EtatEtape::EnCours,
    }
}

/// `None` si le fichier est absent ; sinon contrôle léger de validité (lisible, non vide, signature attendue).
/// La lecture complète vient au chargement des données.
fn verifier(chemin: &Path) -> Option<Result<(), String>> {
    if !chemin.exists() {
        return None;
    }
    let echec = |raison: &str| Some(Err(format!("{} : {raison}", chemin.display())));
    let mut debut = [0u8; 64];
    let lus = match std::fs::File::open(chemin).and_then(|mut f| f.read(&mut debut)) {
        Ok(n) => n,
        Err(e) => return echec(&e.to_string()),
    };
    let debut = &debut[..lus];
    let nom = chemin.to_string_lossy();
    if debut.is_empty() {
        echec("fichier vide")
    } else if nom.ends_with(".gz") && !debut.starts_with(&[0x1f, 0x8b]) {
        echec("pas un fichier gzip")
    } else if nom.ends_with(".gii") && !debut.trim_ascii_start().starts_with(b"<") {
        echec("pas un fichier XML (GIfTI)")
    } else {
        Some(Ok(()))
    }
}

/// Au moins un run `done` → terminée ; sinon un run `running` → en cours ; sinon un run `failed` → erreur.
/// Un `status.json` illisible met l'étape en erreur.
fn etat_simulation(dossier: &Path) -> EtatEtape {
    let mut etats = Vec::new();
    for (_, run) in lister_runs(dossier) {
        let chemin = run.join("status.json");
        if !chemin.exists() {
            continue;
        }
        match lire_etat_run(&chemin) {
            Ok(e) => etats.push((run, e)),
            Err(e) => return EtatEtape::Erreur(e.to_string()),
        }
    }
    let a_l_etat = |etat: State| etats.iter().find(|(_, e)| e.state == etat);
    if a_l_etat(State::Done).is_some() {
        EtatEtape::Terminee
    } else if a_l_etat(State::Running).is_some() {
        EtatEtape::EnCours
    } else if let Some((run, e)) = a_l_etat(State::Failed) {
        let message = e.message.as_deref().unwrap_or("échec");
        EtatEtape::Erreur(format!("{} : {message}", run.display()))
    } else {
        EtatEtape::NonFaite
    }
}
