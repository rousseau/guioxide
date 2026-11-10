//! Tests visuels : chaque onglet du jeu de test fantôme est capturé (`--onglet`, `--capture`) puis comparé à une capture de
//! référence (`tests/references/onglet-N.png`).
//!
//! Ignorés par défaut : il faut le jeu (`scripts/jeu-test/telecharger.sh`), un écran (sous Linux sans écran :
//! `xvfb-run`, avec le pilote Vulkan logiciel lavapipe, `mesa-vulkan-drivers`) et le binaire. Lancement :
//!
//! ```text
//! GUIOXIDE_JEU=~/.cache/guioxide/jeu-test/jeu xvfb-run -a cargo test --release -p guioxide --test visuel -- --ignored
//! GUIOXIDE_MAJ_REFERENCES=1 …   # réécrit les références au lieu de comparer
//! ```
//!
//! **Conditions fixées** pour que la fenêtre soit toujours de 1 400 × 900 pixels et le texte le même : échelle de l'écran à 1
//! (`WINIT_X11_SCALE_FACTOR=1`), opérateur `guioxide-test` (`USER`), et les trente dernières rangées (barre d'état, qui
//! affiche le chemin du jeu) ne sont pas comparées. Les références ont été produites avec lavapipe.
//!
//! **Tolérance** (mesurée le 10 novembre 2026, PGX, jeu fantôme) : deux captures successives du même binaire sont identiques
//! au bit près ; lavapipe contre le GPU NVIDIA (Vulkan) donne, par onglet, 0,0000 % de pixels à plus de 8 niveaux d'écart
//! pour les onglets 1 et 2 (écart maximal de 4 niveaux), 0,0002 % pour l'onglet 4, et 2,7 % (dont 0,8 % à plus de 32 niveaux,
//! contours des surfaces) pour l'onglet 3, qui porte la vue 3D. Seuil par pixel : 32 niveaux sur un canal ; seuil par onglet :
//! 0,1 % de pixels hors tolérance pour les onglets sans 3D, 2 % pour l'onglet 3. Un panneau déplacé, une couleur ou un texte
//! changé dépassent largement 0,1 % d'une fenêtre de 1,26 million de pixels ; un défaut de rendu 3D plus petit que 2 % de la
//! fenêtre n'est pas vu : limite assumée, la vue 3D se juge aussi à l'œil.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Image RVB décodée : largeur, hauteur, octets.
struct Image {
    largeur: usize,
    hauteur: usize,
    octets: Vec<u8>,
}

fn lire_png(chemin: &Path) -> Image {
    let mut decodeur = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(chemin).unwrap()));
    decodeur.set_transformations(png::Transformations::normalize_to_color8());
    let mut lecteur = decodeur.read_info().unwrap();
    let mut tampon = vec![0; lecteur.output_buffer_size().unwrap()];
    let info = lecteur.next_frame(&mut tampon).unwrap();
    let canaux = info.color_type.samples();
    let octets = tampon[..info.buffer_size()]
        .chunks(canaux)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    Image {
        largeur: info.width as usize,
        hauteur: info.height as usize,
        octets,
    }
}

/// Rangées finales ignorées (barre d'état : chemin du jeu, propre à la machine).
const RANGEES_IGNOREES: usize = 30;
/// Écart maximal toléré sur un canal pour qu'un pixel compte comme identique.
const TOLERANCE_PIXEL: u8 = 32;

/// Part des pixels (hors barre d'état) dont un canal diffère de plus de [`TOLERANCE_PIXEL`] ; 1,0 si les tailles diffèrent.
fn part_differente(a: &Image, b: &Image) -> f64 {
    if (a.largeur, a.hauteur) != (b.largeur, b.hauteur) {
        return 1.0;
    }
    let comparees = a.hauteur.saturating_sub(RANGEES_IGNOREES) * a.largeur;
    let differents = a.octets[..comparees * 3]
        .chunks(3)
        .zip(b.octets[..comparees * 3].chunks(3))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > TOLERANCE_PIXEL))
        .count();
    differents as f64 / comparees as f64
}

fn uni(largeur: usize, hauteur: usize, gris: u8) -> Image {
    Image {
        largeur,
        hauteur,
        octets: vec![gris; largeur * hauteur * 3],
    }
}

#[test]
fn deux_images_identiques_ne_different_pas() {
    assert_eq!(part_differente(&uni(100, 80, 40), &uni(100, 80, 40)), 0.0);
}

#[test]
fn un_ecart_sous_la_tolerance_n_est_pas_compte() {
    assert_eq!(
        part_differente(&uni(100, 80, 40), &uni(100, 80, 40 + TOLERANCE_PIXEL)),
        0.0
    );
}

#[test]
fn un_ecart_au_dessus_de_la_tolerance_est_compte() {
    assert_eq!(
        part_differente(&uni(100, 80, 40), &uni(100, 80, 41 + TOLERANCE_PIXEL)),
        1.0
    );
}

#[test]
fn une_zone_changee_est_mesuree_a_sa_surface() {
    let a = uni(100, 130, 40);
    let mut b = uni(100, 130, 40);
    // Carré de 20 × 20 pixels dans la partie comparée (100 rangées) : 4 % des 10 000 pixels.
    for y in 10..30 {
        for x in 10..30 {
            b.octets[(y * 100 + x) * 3] = 200;
        }
    }
    let part = part_differente(&a, &b);
    assert!((part - 0.04).abs() < 1e-9, "{part}");
}

#[test]
fn la_barre_d_etat_et_les_tailles_sont_traitees() {
    let a = uni(100, 130, 40);
    let mut b = uni(100, 130, 40);
    for octet in &mut b.octets[100 * 100 * 3..] {
        *octet = 255; // les trente dernières rangées
    }
    assert_eq!(part_differente(&a, &b), 0.0, "la barre d'état n'est pas comparée");
    assert_eq!(
        part_differente(&uni(100, 130, 40), &uni(100, 131, 40)),
        1.0,
        "tailles différentes"
    );
}

fn jeu() -> PathBuf {
    PathBuf::from(std::env::var_os("GUIOXIDE_JEU").expect("GUIOXIDE_JEU : racine BIDS du jeu de test fantôme"))
}

fn reference(onglet: u8) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/references/onglet-{onglet}.png"))
}

/// Une seule fenêtre à la fois : plusieurs captures simultanées se disputent l'écran et le focus.
static FENETRE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Capture un onglet du jeu dans les conditions fixées et la compare à sa référence.
fn verifier_onglet(onglet: u8, part_max: f64) {
    let _fenetre = FENETRE.lock().unwrap_or_else(|e| e.into_inner());
    let dossier = std::env::temp_dir().join(format!("guioxide-visuel-{}-{onglet}", std::process::id()));
    std::fs::create_dir_all(&dossier).unwrap();
    let capture = dossier.join(format!("onglet-{onglet}.png"));
    let sortie = Command::new(env!("CARGO_BIN_EXE_guioxide"))
        .arg(jeu())
        .args(["--onglet", &onglet.to_string(), "--capture"])
        .arg(&capture)
        .env("WINIT_X11_SCALE_FACTOR", "1")
        .env("USER", "guioxide-test")
        .output()
        .unwrap();
    assert!(
        sortie.status.success(),
        "capture de l'onglet {onglet} : {}",
        String::from_utf8_lossy(&sortie.stderr)
    );
    if std::env::var_os("GUIOXIDE_MAJ_REFERENCES").is_some() {
        std::fs::copy(&capture, reference(onglet)).unwrap();
        return;
    }
    let part = part_differente(&lire_png(&capture), &lire_png(&reference(onglet)));
    assert!(
        part <= part_max,
        "onglet {onglet} : {:.3} % de pixels hors tolérance (au plus {:.3} %) ; capture : {}",
        part * 100.0,
        part_max * 100.0,
        capture.display()
    );
    let _ = std::fs::remove_dir_all(&dossier);
}

#[test]
#[ignore = "jeu de test et écran"]
fn onglet_1_examen() {
    verifier_onglet(1, 0.001);
}

#[test]
#[ignore = "jeu de test et écran"]
fn onglet_2_reconstruction() {
    verifier_onglet(2, 0.001);
}

#[test]
#[ignore = "jeu de test et écran"]
fn onglet_3_surfaces() {
    verifier_onglet(3, 0.02);
}

#[test]
#[ignore = "jeu de test et écran"]
fn onglet_4_simulation() {
    verifier_onglet(4, 0.001);
}
