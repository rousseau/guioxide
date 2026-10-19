//! Lecture de petits NIfTI écrits à la main : ordre des axes, affine sform ou qform, chargeur et cache.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use guioxide_data::{Chargeur, Etat, lire_volume};

/// En-tête NIfTI-1 minimal (348 octets + 4 d'extension), voxels `f32` à la suite.
/// `sform` : lignes de l'affine, ou `None` pour un qform seul (identité tournée de 180° autour de z, voxels 2 mm).
fn ecrire_nifti(chemin: &Path, dims: [usize; 3], voxels: &[f32], sform: Option<[[f32; 4]; 3]>) {
    let mut h = vec![0u8; 352];
    let i32_ = |h: &mut Vec<u8>, o: usize, v: i32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let i16_ = |h: &mut Vec<u8>, o: usize, v: i16| h[o..o + 2].copy_from_slice(&v.to_le_bytes());
    let f32_ = |h: &mut Vec<u8>, o: usize, v: f32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
    i32_(&mut h, 0, 348);
    for (n, d) in [3, dims[0], dims[1], dims[2], 1, 1, 1, 1].iter().enumerate() {
        i16_(&mut h, 40 + 2 * n, *d as i16);
    }
    i16_(&mut h, 70, 16); // float32
    i16_(&mut h, 72, 32);
    for (n, p) in [1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0].iter().enumerate() {
        f32_(&mut h, 76 + 4 * n, *p);
    }
    f32_(&mut h, 108, 352.0);
    f32_(&mut h, 112, 1.0); // scl_slope
    match sform {
        Some(lignes) => {
            i16_(&mut h, 254, 1);
            for (r, ligne) in lignes.iter().enumerate() {
                for (c, v) in ligne.iter().enumerate() {
                    f32_(&mut h, 280 + 16 * r + 4 * c, *v);
                }
            }
        }
        None => {
            // Quaternion (b, c, d) = (0, 0, 1) : rotation de 180° autour de z ; translation (10, 20, 30).
            i16_(&mut h, 252, 1);
            f32_(&mut h, 264, 1.0);
            for (n, t) in [10.0, 20.0, 30.0].iter().enumerate() {
                f32_(&mut h, 268 + 4 * n, *t);
            }
        }
    }
    h[344..348].copy_from_slice(b"n+1\0");
    for v in voxels {
        h.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(chemin, h).unwrap();
}

/// Voxels `i + 10·j + 100·k`, écrits dans l'ordre du fichier (i le plus rapide).
fn voxels(dims: [usize; 3]) -> Vec<f32> {
    let mut v = Vec::new();
    for k in 0..dims[2] {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                v.push((i + 10 * j + 100 * k) as f32);
            }
        }
    }
    v
}

#[test]
fn ordre_des_axes_et_affine_sform() {
    let dir = tempfile::tempdir().unwrap();
    let chemin = dir.path().join("oblique.nii");
    let dims = [4, 3, 2];
    let sform = [[-1.2, 0.0, 0.0, 155.0], [0.0, -1.0, 0.8, 58.0], [0.0, 0.5, 1.8, -254.0]];
    ecrire_nifti(&chemin, dims, &voxels(dims), Some(sform));
    let lu = lire_volume(&chemin).unwrap();
    assert_eq!(lu.source_affine, "sform");
    assert_eq!(lu.volume.dims(), dims);
    for (i, j, k) in [(0, 0, 0), (3, 0, 0), (0, 2, 0), (1, 2, 1), (3, 2, 1)] {
        assert_eq!(
            lu.volume.valeur(i, j, k),
            (i + 10 * j + 100 * k) as f32,
            "({i}, {j}, {k})"
        );
    }
    let p = lu.volume.vers_monde([1.0, 2.0, 1.0]);
    let attendu = [155.0 - 1.2, 58.0 - 2.0 + 0.8, -254.0 + 1.0 + 1.8];
    assert!((0..3).all(|c| (p[c] - attendu[c]).abs() < 1e-4), "{p:?}");
}

#[test]
fn affine_qform_seul() {
    let dir = tempfile::tempdir().unwrap();
    let chemin = dir.path().join("qform.nii");
    ecrire_nifti(&chemin, [2, 2, 2], &voxels([2, 2, 2]), None);
    let lu = lire_volume(&chemin).unwrap();
    assert_eq!(lu.source_affine, "qform");
    // Rotation de 180° autour de z : x et y changent de signe ; voxels de 2 mm.
    assert_eq!(lu.volume.vers_monde([1.0, 1.0, 1.0]), [8.0, 18.0, 32.0]);
}

#[test]
fn erreur_avec_le_chemin() {
    let dir = tempfile::tempdir().unwrap();
    let chemin = dir.path().join("casse.nii.gz");
    std::fs::write(&chemin, b"pas un nifti").unwrap();
    let e = lire_volume(&chemin).unwrap_err();
    assert!(e.to_string().contains("casse.nii.gz"), "{e}");
}

#[test]
fn chargeur_en_arriere_plan_et_eviction() {
    let dir = tempfile::tempdir().unwrap();
    let dims = [10, 10, 10]; // 4 000 octets de voxels
    let chemins: Vec<_> = (0..3).map(|n| dir.path().join(format!("v{n}.nii"))).collect();
    for c in &chemins {
        ecrire_nifti(
            c,
            dims,
            &voxels(dims),
            Some([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]),
        );
    }
    let (reveil, attente) = mpsc::channel();
    let mut chargeur = Chargeur::volumes(9_000, move || reveil.send(()).unwrap());
    let pret = |chargeur: &mut Chargeur<_>, chemin: &Path| {
        for _ in 0..2 {
            match chargeur.demander(chemin) {
                Etat::Pret(v) => return v,
                Etat::EnCours => attente.recv_timeout(Duration::from_secs(10)).unwrap(),
                Etat::Erreur(e) => panic!("{e}"),
            }
        }
        match chargeur.demander(chemin) {
            Etat::Pret(v) => v,
            e => panic!("{e:?}"),
        }
    };
    pret(&mut chargeur, &chemins[0]);
    pret(&mut chargeur, &chemins[1]);
    assert_eq!(chargeur.occupation(), 8_000);
    pret(&mut chargeur, &chemins[0]); // v0 redevient le plus récent
    pret(&mut chargeur, &chemins[2]); // dépasse 9 000 : v1, le moins récemment demandé, est évincé
    assert_eq!(chargeur.occupation(), 8_000);
    assert!(matches!(chargeur.demander(&chemins[0]), Etat::Pret(_)));
    assert!(
        matches!(chargeur.demander(&chemins[1]), Etat::EnCours),
        "v1 a été évincé : relu"
    );

    assert!(matches!(
        chargeur.demander(&dir.path().join("absent.nii")),
        Etat::EnCours
    ));
    attente.recv_timeout(Duration::from_secs(10)).unwrap();
    attente.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(matches!(chargeur.demander(&dir.path().join("absent.nii")), Etat::Erreur(e) if e.contains("absent.nii")));
}
