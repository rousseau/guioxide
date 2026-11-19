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
//! **Conditions fixées** pour que la fenêtre soit toujours de 1 400 × 900 pixels et le texte le même : en mode `--capture`,
//! guioxide ignore l'état mémorisé (`app.ron`) et capture à un pixel par point quelle que soit l'échelle de l'écran ; les
//! tests y ajoutent un dossier personnel vierge (`HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`), l'échelle 1 sous X11
//! (`WINIT_X11_SCALE_FACTOR=1`) et l'opérateur `guioxide-test` (`USER`) ; les trente dernières rangées (barre d'état, qui
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

/// Dossier personnel vierge propre à une capture : `HOME`, `XDG_DATA_HOME` et `XDG_CONFIG_HOME` y pointent, pour que
/// l'état d'un usage réel (`app.ron` : taille de fenêtre, récents, opérateur) ne puisse pas changer la capture, sous Linux
/// (`~/.local/share/guioxide`) comme sous macOS (`~/Library/Application Support/guioxide`).
fn isoler(commande: &mut Command, home: &Path) {
    commande
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CONFIG_HOME", home.join(".config"));
    // L'autorisation X11 se cherche dans `~/.Xauthority` quand `XAUTHORITY` n'est pas fixé : garder la vraie.
    if std::env::var_os("XAUTHORITY").is_none()
        && let Some(vraie) = std::env::home_dir()
            .map(|h| h.join(".Xauthority"))
            .filter(|f| f.exists())
    {
        commande.env("XAUTHORITY", vraie);
    }
}

/// Capture un onglet du jeu dans les conditions fixées, dans un dossier temporaire qu'il rend ; `regler` complète
/// l'environnement (après l'isolement).
fn capturer(onglet: u8, regler: impl FnOnce(&mut Command, &Path)) -> (tempfile::TempDir, PathBuf) {
    let dossier = tempfile::Builder::new()
        .prefix(&format!("guioxide-visuel-{onglet}-"))
        .tempdir()
        .unwrap();
    let home = dossier.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let capture = dossier.path().join(format!("onglet-{onglet}.png"));
    let mut commande = Command::new(env!("CARGO_BIN_EXE_guioxide"));
    commande
        .arg(jeu())
        .args(["--onglet", &onglet.to_string(), "--capture"])
        .arg(&capture)
        .env("WINIT_X11_SCALE_FACTOR", "1")
        .env("USER", "guioxide-test");
    isoler(&mut commande, &home);
    regler(&mut commande, &home);
    let sortie = commande.output().unwrap();
    assert!(
        sortie.status.success(),
        "capture de l'onglet {onglet} : {}",
        String::from_utf8_lossy(&sortie.stderr)
    );
    (dossier, capture)
}

/// Compare une capture à la référence de son onglet ; garde la capture (dossier conservé) si elle s'en écarte.
fn comparer(onglet: u8, part_max: f64, (dossier, capture): (tempfile::TempDir, PathBuf)) {
    let image = lire_png(&capture);
    let part = part_differente(&image, &lire_png(&reference(onglet)));
    if part > part_max {
        let _ = dossier.keep();
    }
    assert!(
        part <= part_max,
        "onglet {onglet} : {:.3} % de pixels hors tolérance (au plus {:.3} %), capture de {} × {} : {}",
        part * 100.0,
        part_max * 100.0,
        image.largeur,
        image.hauteur,
        capture.display()
    );
}

/// Capture un onglet du jeu dans les conditions fixées et la compare à sa référence.
fn verifier_onglet(onglet: u8, part_max: f64) {
    let _fenetre = FENETRE.lock().unwrap_or_else(|e| e.into_inner());
    let resultat = capturer(onglet, |_, _| {});
    if std::env::var_os("GUIOXIDE_MAJ_REFERENCES").is_some() {
        std::fs::copy(&resultat.1, reference(onglet)).unwrap();
        return;
    }
    comparer(onglet, part_max, resultat);
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

/// État d'un usage réel (E42, sur la RTX : fenêtre de 1 841 × 1 080 mémorisée, opérateur) placé là où l'utilisateur
/// l'aurait, sous Linux et sous macOS, sans `XDG_DATA_HOME` : la capture n'en tient pas compte.
#[test]
#[ignore = "jeu de test et écran"]
fn etat_de_l_utilisateur_ignore() {
    let _fenetre = FENETRE.lock().unwrap_or_else(|e| e.into_inner());
    let app_ron = r#"{
    "window": "(normal_geometry:(outer_position_pixels:Some((x:0.0,y:0.0)),scale_factor:1.0,inner_size_points:(x:1841.0,y:1080.0)),fullscreen:false,maximized:false)",
    "operateur": "\"utilisateur-reel\"",
}"#;
    let resultat = capturer(1, |commande, home| {
        for dossier in [".local/share/guioxide", "Library/Application Support/guioxide"] {
            std::fs::create_dir_all(home.join(dossier)).unwrap();
            std::fs::write(home.join(dossier).join("app.ron"), app_ron).unwrap();
        }
        commande.env_remove("XDG_DATA_HOME");
    });
    let image = lire_png(&resultat.1);
    assert_eq!((image.largeur, image.hauteur), (1400, 900), "taille de la capture");
    comparer(1, 0.001, resultat);
}

/// Écran à l'échelle 2 (Retina ; simulé sous X11 par `WINIT_X11_SCALE_FACTOR`) : la capture garde 1 400 × 900 pixels et
/// le rendu de l'échelle 1.
#[test]
#[ignore = "jeu de test et écran"]
fn ecran_a_l_echelle_2() {
    let _fenetre = FENETRE.lock().unwrap_or_else(|e| e.into_inner());
    let resultat = capturer(1, |commande, _| {
        commande.env("WINIT_X11_SCALE_FACTOR", "2");
    });
    let image = lire_png(&resultat.1);
    assert_eq!((image.largeur, image.hauteur), (1400, 900), "taille de la capture");
    comparer(1, 0.001, resultat);
}
