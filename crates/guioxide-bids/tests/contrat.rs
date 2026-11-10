//! Contrat de données (`docs/donnees.md`) sur le jeu de test fantôme : chaque fichier marqué « existant » est lu sans erreur
//! par les lecteurs de guioxide, et l'état d'avancement des quatre étapes est « terminé ».
//!
//! Ignoré par défaut : il faut le jeu (`scripts/jeu-test/telecharger.sh`, ou `construire.sh`). Lancement :
//!
//! ```text
//! GUIOXIDE_JEU=~/.cache/guioxide/jeu-test/jeu cargo test --release -p guioxide-bids --test contrat -- --ignored
//! ```

use std::path::{Path, PathBuf};

use guioxide_bids::{Etape, EtatEtape, etat_etape, indexer};
use guioxide_data::{
    TableEtiquettes, lire_champ, lire_controle_surfaces, lire_etat_run, lire_mouvement, lire_qualite, lire_sequence,
    lire_surface, lire_volume, lister_runs,
};

/// Version du contrat que ce test vérifie : celle de `docs/donnees.md` (« Version du contrat »).
const VERSION_CONTRAT: &str = "1.0";

fn jeu() -> PathBuf {
    let chemin = std::env::var_os("GUIOXIDE_JEU").expect("GUIOXIDE_JEU : racine BIDS du jeu de test fantôme");
    PathBuf::from(chemin)
}

/// Le `anat/` d'une dérivée du jeu : `derivatives/<pipeline>/sub-fant01/ses-01/anat/`.
fn anat(racine: &Path, pipeline: &str) -> PathBuf {
    racine.join("derivatives").join(pipeline).join("sub-fant01/ses-01/anat")
}

const PREFIXE: &str = "sub-fant01_ses-01";

#[test]
fn la_version_du_contrat_est_celle_de_la_documentation() {
    let doc = include_str!("../../../docs/donnees.md");
    assert!(
        doc.contains(&format!("**Version du contrat : {VERSION_CONTRAT}**")),
        "docs/donnees.md doit annoncer « **Version du contrat : {VERSION_CONTRAT}** »"
    );
}

#[test]
#[ignore = "jeu de test"]
fn chaque_derivee_declare_son_generateur() {
    let racine = jeu();
    for pipeline in ["medx-fetalbet", "medx-svr", "medx-bounti", "surfx-surfaces", "gyroxide"] {
        let chemin = racine
            .join("derivatives")
            .join(pipeline)
            .join("dataset_description.json");
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&chemin).unwrap()).unwrap();
        assert_eq!(json["DatasetType"], "derivative", "{}", chemin.display());
        let generateur = &json["GeneratedBy"][0];
        assert!(
            generateur["Name"].is_string(),
            "{} : GeneratedBy.Name",
            chemin.display()
        );
        assert!(
            generateur["Version"].is_string(),
            "{} : GeneratedBy.Version",
            chemin.display()
        );
    }
}

#[test]
#[ignore = "jeu de test"]
fn les_fichiers_existants_du_contrat_se_lisent() {
    let racine = jeu();

    // Étape 1 : masques des piles, qualité avant reconstruction.
    let masques = anat(&racine, "medx-fetalbet");
    let mut nb_masques = 0;
    for entree in std::fs::read_dir(&masques).unwrap() {
        let chemin = entree.unwrap().path();
        if chemin.to_string_lossy().ends_with("_desc-brain_mask.nii.gz") {
            lire_volume(&chemin).unwrap();
            nb_masques += 1;
        }
    }
    assert_eq!(nb_masques, 3, "un masque par pile");
    let svr = anat(&racine, "medx-svr");
    assert!(
        !lire_qualite(&svr.join(format!("{PREFIXE}_desc-acq_slicequality.tsv")))
            .unwrap()
            .is_empty()
    );

    // Étape 2 : volume reconstruit, mouvement, qualité après, segmentation et sa table.
    lire_volume(&svr.join(format!("{PREFIXE}_desc-svr_T2w.nii.gz"))).unwrap();
    assert!(
        !lire_mouvement(&svr.join(format!("{PREFIXE}_desc-svr_motion.tsv")))
            .unwrap()
            .is_empty()
    );
    assert!(
        !lire_qualite(&svr.join(format!("{PREFIXE}_desc-svr_slicequality.tsv")))
            .unwrap()
            .is_empty()
    );
    let bounti = anat(&racine, "medx-bounti");
    lire_volume(&bounti.join(format!("{PREFIXE}_desc-bounti_dseg.nii.gz"))).unwrap();
    lire_volume(&bounti.join(format!("{PREFIXE}_desc-bounti_mask.nii.gz"))).unwrap();
    let table = TableEtiquettes::lire(&bounti.join(format!("{PREFIXE}_desc-bounti_dseg.tsv"))).unwrap();
    assert!(table.couleur(1) != [0, 0, 0] || table.couleur(2) != [0, 0, 0]);

    // Étape 3 : surfaces, cartes par sommet, contrôle des maillages.
    let surfaces = anat(&racine, "surfx-surfaces");
    for hemi in ["L", "R"] {
        let blanche = lire_surface(&surfaces.join(format!("{PREFIXE}_hemi-{hemi}_white.surf.gii"))).unwrap();
        let piale = lire_surface(&surfaces.join(format!("{PREFIXE}_hemi-{hemi}_pial.surf.gii"))).unwrap();
        assert_eq!(
            blanche.sommets.len(),
            piale.sommets.len(),
            "blanche et piale : mêmes sommets"
        );
        for champ in ["curv", "sulc", "thickness", "cortex"] {
            let valeurs = lire_champ(&surfaces.join(format!("{PREFIXE}_hemi-{hemi}_{champ}.shape.gii"))).unwrap();
            assert_eq!(valeurs.len(), blanche.sommets.len(), "{champ} : une valeur par sommet");
        }
    }
    assert_eq!(
        lire_controle_surfaces(&surfaces.join(format!("{PREFIXE}_desc-surfqc.tsv")))
            .unwrap()
            .len(),
        4
    );

    // Étape 4 : runs de gyroxide et maillage tétraédrique.
    let sim = anat(&racine, "gyroxide");
    let runs = lister_runs(&sim);
    assert_eq!(runs.len(), 1, "un run court dans le jeu");
    let (_, dossier) = &runs[0];
    lire_etat_run(&dossier.join("status.json")).unwrap();
    assert!(!lire_sequence(&dossier.join("sequence.tsv")).unwrap().is_empty());
    assert!(sim.join(format!("{PREFIXE}_hemi-L_desc-tetgen_tet.vtu")).is_file());
}

#[test]
#[ignore = "jeu de test"]
fn le_jeu_est_indexe_et_les_quatre_etapes_sont_terminees() {
    let racine = jeu();
    let jeu = indexer(&racine).unwrap();
    assert_eq!(jeu.sujets.len(), 1);
    let sujet = &jeu.sujets[0];
    let session = &sujet.sessions[0];
    assert_eq!(session.piles.len(), 3);
    assert_eq!(session.age_gestationnel, Some(32.0));
    let derivees = racine.join("derivatives");
    for etape in Etape::TOUTES {
        let etat = etat_etape(etape, &derivees, sujet, session);
        assert!(matches!(etat, EtatEtape::Terminee), "{} : {etat:?}", etape.nom());
    }
}
