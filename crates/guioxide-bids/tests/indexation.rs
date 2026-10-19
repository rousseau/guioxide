//! Indexation et état d'avancement sur des arborescences factices, dont des cas invalides.

use std::fs;
use std::path::Path;

use guioxide_bids::{ErreurBids, Etape, EtatEtape, etat_etape, indexer, trouver_racine};

const GZIP: &[u8] = &[0x1f, 0x8b, 0x08, 0x00];
const GIFTI: &[u8] = b"<?xml version=\"1.0\"?>\n<GIFTI/>";

fn ecrire(chemin: &Path, contenu: &[u8]) {
    fs::create_dir_all(chemin.parent().unwrap()).unwrap();
    fs::write(chemin, contenu).unwrap();
}

/// Bucket « à la Garage » : `sourcedata/` (racine BIDS) et `derivatives/` à côté.
///
/// - sub-01 : ses-01 (deux piles T2w, AG 30,5), ses-02 (une T2w et une T1w, AG absent) ;
/// - sub-02 : sans session ; participants.tsv donne son AG ;
/// - sub-03 : listé dans participants.tsv, sans dossier ; sub-04 : dossier non listé, vide.
fn bucket(racine: &Path) {
    let src = racine.join("sourcedata");
    ecrire(
        &src.join("dataset_description.json"),
        br#"{"Name": "Jeu factice", "BIDSVersion": "1.9.0"}"#,
    );
    ecrire(
        &src.join("participants.tsv"),
        b"participant_id\tga_weeks\nsub-01\tn/a\nsub-02\t27.0\nsub-03\t30\n",
    );
    ecrire(
        &src.join("sub-01/sub-01_sessions.tsv"),
        b"session_id\tga_weeks\nses-01\t30.5\nses-02\tn/a\n",
    );
    for f in [
        "ses-01/anat/sub-01_ses-01_run-1_T2w",
        "ses-01/anat/sub-01_ses-01_run-2_T2w",
    ] {
        ecrire(&src.join(format!("sub-01/{f}.nii.gz")), GZIP);
    }
    ecrire(&src.join("sub-01/ses-01/anat/sub-01_ses-01_run-1_T2w.json"), b"{}");
    ecrire(&src.join("sub-01/ses-02/anat/sub-01_ses-02_T2w.nii.gz"), GZIP);
    ecrire(&src.join("sub-01/ses-02/anat/sub-01_ses-02_T1w.nii"), b"nifti");
    ecrire(&src.join("sub-02/anat/sub-02_T2w.nii.gz"), GZIP);
    fs::create_dir_all(src.join("sub-04")).unwrap();
    ecrire(
        &racine.join("derivatives/medx-svr/dataset_description.json"),
        br#"{"Name": "SVR", "DatasetType": "derivative", "GeneratedBy": [{"Name": "medx", "Version": "0.1.0"}]}"#,
    );
    fs::create_dir_all(racine.join("derivatives/sans-description")).unwrap();
}

#[test]
fn racine_trouvee_depuis_le_bucket_la_racine_bids_ou_un_sous_dossier() {
    let tmp = tempfile::tempdir().unwrap();
    bucket(tmp.path());
    let src = tmp.path().join("sourcedata").canonicalize().unwrap();
    for depart in ["", "sourcedata", "sourcedata/sub-01", "sourcedata/sub-01/ses-01/anat"] {
        assert_eq!(
            trouver_racine(&tmp.path().join(depart)).unwrap(),
            src,
            "depuis « {depart} »"
        );
    }
    let fichier = tmp.path().join("sourcedata/sub-02/anat/sub-02_T2w.nii.gz");
    assert_eq!(trouver_racine(&fichier).unwrap(), src);
}

#[test]
fn le_dataset_description_d_une_derivee_n_est_pas_une_racine() {
    let tmp = tempfile::tempdir().unwrap();
    let racine = tmp.path();
    ecrire(&racine.join("dataset_description.json"), br#"{"Name": "Plat"}"#);
    ecrire(
        &racine.join("derivatives/medx-svr/dataset_description.json"),
        br#"{"DatasetType": "derivative"}"#,
    );
    fs::create_dir_all(racine.join("derivatives/medx-svr/sub-01")).unwrap();
    let trouvee = trouver_racine(&racine.join("derivatives/medx-svr/sub-01")).unwrap();
    assert_eq!(trouvee, racine.canonicalize().unwrap());
}

#[test]
fn indexation_du_bucket() {
    let tmp = tempfile::tempdir().unwrap();
    bucket(tmp.path());
    let jeu = indexer(tmp.path()).unwrap();
    assert_eq!(jeu.nom, "Jeu factice");
    assert_eq!(jeu.version_bids.as_deref(), Some("1.9.0"));
    assert_eq!(jeu.racine_etude, tmp.path().canonicalize().unwrap());

    let ids: Vec<_> = jeu.sujets.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["sub-01", "sub-02", "sub-04"]);
    assert_eq!(jeu.nb_sessions(), 4);

    let s1 = &jeu.sujets[0];
    let ses: Vec<_> = s1.sessions.iter().map(|s| s.id.as_deref()).collect();
    assert_eq!(ses, [Some("ses-01"), Some("ses-02")]);
    assert_eq!(s1.sessions[0].age_gestationnel, Some(30.5));
    assert_eq!(s1.sessions[1].age_gestationnel, None);
    let noms: Vec<_> = s1.sessions[0].piles.iter().map(|p| p.nom.as_str()).collect();
    assert_eq!(
        noms,
        ["sub-01_ses-01_run-1_T2w", "sub-01_ses-01_run-2_T2w"],
        "le .json voisin n'est pas une pile"
    );
    assert_eq!(
        s1.sessions[1].contrastes().into_iter().collect::<Vec<_>>(),
        ["T1w", "T2w"]
    );

    let s2 = &jeu.sujets[1].sessions[0];
    assert_eq!(s2.id, None);
    assert_eq!(s2.age_gestationnel, Some(27.0));
    assert_eq!(s2.prefixe(&jeu.sujets[1]), "sub-02");
    assert_eq!(s1.sessions[0].prefixe(s1), "sub-01_ses-01");

    assert_eq!(jeu.contrastes().into_iter().collect::<Vec<_>>(), ["T1w", "T2w"]);
    let attendus = [
        "sub-04 : ni session ni image anat",
        "sub-03 : listé dans participants.tsv, sans dossier",
        "sub-04 : absent de participants.tsv",
    ];
    assert_eq!(jeu.avertissements, attendus);

    let derivees: Vec<_> = jeu
        .derivees
        .iter()
        .map(|d| (d.pipeline.as_str(), d.generateur.as_deref()))
        .collect();
    assert_eq!(derivees, [("medx-svr", Some("medx 0.1.0")), ("sans-description", None)]);
}

#[test]
fn cas_invalides() {
    let tmp = tempfile::tempdir().unwrap();
    let vide = tmp.path().join("vide");
    fs::create_dir_all(&vide).unwrap();
    assert!(matches!(indexer(&vide), Err(ErreurBids::RacineIntrouvable(_))));
    assert!(matches!(
        indexer(&tmp.path().join("absent")),
        Err(ErreurBids::Lecture { .. })
    ));

    let casse = tmp.path().join("casse");
    ecrire(&casse.join("dataset_description.json"), b"{ pas du json");
    let erreur = indexer(&casse).unwrap_err();
    assert!(matches!(erreur, ErreurBids::Format { .. }));
    assert!(
        erreur.to_string().contains("dataset_description.json"),
        "le message cite le fichier : {erreur}"
    );
}

#[test]
fn etats_d_avancement_selon_le_contrat() {
    let tmp = tempfile::tempdir().unwrap();
    bucket(tmp.path());
    let jeu = indexer(tmp.path()).unwrap();
    let der = jeu.derivees_etude();
    let (sujet, session) = (&jeu.sujets[0], &jeu.sujets[0].sessions[0]);
    let anat = |pipeline: &str| der.join(pipeline).join("sub-01/ses-01/anat");
    let etat = |e| etat_etape(e, &der, sujet, session);

    for e in Etape::TOUTES {
        assert_eq!(etat(e), EtatEtape::NonFaite, "{e}");
    }

    // 1 · Examen : un masque par pile T2w.
    ecrire(
        &anat("medx-fetalbet").join("sub-01_ses-01_run-1_T2w_desc-brain_mask.nii.gz"),
        GZIP,
    );
    assert_eq!(etat(Etape::Examen), EtatEtape::EnCours);
    ecrire(
        &anat("medx-fetalbet").join("sub-01_ses-01_run-2_T2w_desc-brain_mask.nii.gz"),
        GZIP,
    );
    assert_eq!(etat(Etape::Examen), EtatEtape::Terminee);

    // 2 · Reconstruction : volume SVR et segmentation.
    ecrire(&anat("medx-svr").join("sub-01_ses-01_desc-svr_T2w.nii.gz"), GZIP);
    assert_eq!(etat(Etape::Reconstruction), EtatEtape::EnCours);
    ecrire(
        &anat("medx-bounti").join("sub-01_ses-01_desc-bounti_dseg.nii.gz"),
        b"pas du gzip",
    );
    assert!(matches!(etat(Etape::Reconstruction), EtatEtape::Erreur(m) if m.contains("desc-bounti_dseg")));
    ecrire(&anat("medx-bounti").join("sub-01_ses-01_desc-bounti_dseg.nii.gz"), GZIP);
    assert_eq!(etat(Etape::Reconstruction), EtatEtape::Terminee);

    // 3 · Surfaces : blanche et piale par hémisphère, ou sans `hemi-`.
    let surf = anat("surfx-surfaces");
    ecrire(&surf.join("sub-01_ses-01_hemi-L_white.surf.gii"), GIFTI);
    ecrire(&surf.join("sub-01_ses-01_hemi-L_pial.surf.gii"), GIFTI);
    assert_eq!(etat(Etape::Surfaces), EtatEtape::EnCours);
    ecrire(&surf.join("sub-01_ses-01_hemi-R_white.surf.gii"), GIFTI);
    ecrire(&surf.join("sub-01_ses-01_hemi-R_pial.surf.gii"), GIFTI);
    assert_eq!(etat(Etape::Surfaces), EtatEtape::Terminee);

    // 4 · Simulation : état des runs.
    let gyro = anat("gyroxide");
    ecrire(
        &gyro.join("run-a/status.json"),
        br#"{"state": "failed", "message": "NaN au pas 120"}"#,
    );
    assert!(matches!(etat(Etape::Simulation), EtatEtape::Erreur(m) if m.contains("NaN au pas 120")));
    ecrire(&gyro.join("run-b/status.json"), br#"{"state": "running", "step": 10}"#);
    assert_eq!(etat(Etape::Simulation), EtatEtape::EnCours);
    ecrire(&gyro.join("run-c/status.json"), br#"{"state": "done"}"#);
    assert_eq!(etat(Etape::Simulation), EtatEtape::Terminee);
    ecrire(&gyro.join("run-d/status.json"), b"{ tronque");
    assert!(matches!(etat(Etape::Simulation), EtatEtape::Erreur(m) if m.contains("run-d")));
}

#[test]
fn surfaces_sans_hemisphere_et_sortie_libre() {
    let tmp = tempfile::tempdir().unwrap();
    bucket(tmp.path());
    let jeu = indexer(tmp.path()).unwrap();
    let (sujet, session) = (&jeu.sujets[1], &jeu.sujets[1].sessions[0]);

    // Une sortie libre n'est pas l'étude : l'état est lu dans son propre `derivatives/`.
    let libre = tmp.path().join("essai/derivatives");
    let surf = libre.join("surfx-surfaces/sub-02/anat");
    ecrire(&surf.join("sub-02_white.surf.gii"), GIFTI);
    ecrire(&surf.join("sub-02_pial.surf.gii"), GIFTI);
    assert_eq!(etat_etape(Etape::Surfaces, &libre, sujet, session), EtatEtape::Terminee);
    assert_eq!(
        etat_etape(Etape::Surfaces, &jeu.derivees_etude(), sujet, session),
        EtatEtape::NonFaite
    );

    // Une session sans pile T2w n'a pas d'examen à faire.
    let sujet4 = &jeu.sujets[2];
    assert_eq!(
        etat_etape(Etape::Examen, &libre, sujet4, &sujet4.sessions[0]),
        EtatEtape::NonFaite
    );
}
