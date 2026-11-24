//! guioxide : interface graphique de la suite oxide pour l'IRM fœtale.
//!
//! ```text
//! guioxide [chemin du jeu BIDS] [--onglet 1-4] [--session <texte>] [--capture <fichier.png>]
//! guioxide <pile.nii.gz>…
//! ```
//!
//! - `--calculs` : ouvre la fenêtre « Calculs » de la session ;
//! - des fichiers NIfTI à la place du jeu : fenêtre « Nouvelle séance » ouverte avec ces piles ;
//! - `--onglet` : ouvre directement une étape (1 Examen … 4 Simulation) ;
//! - `--session` : choisit la session dont le libellé contient ce texte (`sub-01 / ses-02`), la première sinon ;
//! - `--capture` : enregistre la fenêtre en PNG une fois les données affichées, puis quitte (erreur après 5 min si
//!   la fenêtre ne se redessine pas : écran verrouillé, fenêtre masquée).

mod app;
mod calculs;
mod capture;
mod chaine;
mod dialogue;
mod etapes;
mod examen;
mod import;
mod ouverture;
mod reconstruction;
mod simulation;
mod surfaces;

use std::path::PathBuf;

use guioxide_bids::Etape;

const USAGE: &str = "usage : guioxide [chemin] [--onglet 1-4] [--session <texte>] [--capture <fichier.png>]
        guioxide <pile.nii.gz>…   (nouvelle séance à partir de ces piles)";

fn options() -> Result<app::Options, String> {
    let mut options = app::Options::default();
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        let mut valeur = |nom: &str| args.next().ok_or_else(|| format!("{nom} attend une valeur\n{USAGE}"));
        match a.to_str() {
            Some("--onglet") => {
                let v = valeur("--onglet")?;
                let n: u8 = v
                    .to_str()
                    .and_then(|t| t.parse().ok())
                    .ok_or(format!("--onglet : 1 à 4\n{USAGE}"))?;
                options.onglet = Some(Etape::depuis_numero(n).ok_or(format!("--onglet : 1 à 4\n{USAGE}"))?);
            }
            Some("--session") => options.session = Some(valeur("--session")?.to_string_lossy().into_owned()),
            Some("--calculs") => options.calculs = true,
            Some("--capture") => options.capture = Some(PathBuf::from(valeur("--capture")?)),
            Some("-h" | "--help") => return Err(USAGE.to_owned()),
            Some(o) if o.starts_with("--") => return Err(format!("option inconnue : {o}\n{USAGE}")),
            _ if import::est_nifti(std::path::Path::new(&a)) => options.piles.push(PathBuf::from(a)),
            _ if options.jeu.is_none() => options.jeu = Some(PathBuf::from(a)),
            _ => return Err(format!("un seul jeu à la fois\n{USAGE}")),
        }
    }
    Ok(options)
}

/// Icône de l'application (Dock, barre des tâches) ; sans elle, eframe montre celle d'egui. Dessinée par
/// `scripts/macos/icone.py` à partir du logo (`assets/logo.png`).
fn icone() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icone.png")).expect("icône PNG intégrée")
}

fn main() -> eframe::Result {
    let options_guioxide = match options() {
        Ok(o) => o,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("guioxide")
            .with_icon(icone())
            .with_inner_size(capture::TAILLE_FENETRE)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    // `--capture` : rien de l'usage réel (taille et place de la fenêtre, récents, opérateur, dans `app.ron`) ne doit
    // changer l'image. L'état se lit et s'écrit dans un dossier temporaire, effacé à la sortie, et la fenêtre n'est
    // pas mémorisée ; pas de taille minimale, qui sur un écran à l'échelle 2 dépasserait la taille de capture.
    let etat_capture = options_guioxide.capture.as_ref().map(|chemin| {
        capture::surveiller(chemin.clone());
        std::env::temp_dir().join(format!("guioxide-capture-{}", std::process::id()))
    });
    match &etat_capture {
        Some(dossier) => {
            options.persistence_path = Some(dossier.join("app.ron"));
            options.persist_window = false;
        }
        None => options.viewport = options.viewport.with_min_inner_size([900.0, 600.0]),
    }
    let resultat = eframe::run_native(
        "guioxide",
        options,
        Box::new(|cc| {
            guioxide_view::theme::appliquer(&cc.egui_ctx);
            guioxide_view::polices::installer(&cc.egui_ctx);
            Ok(Box::new(app::Application::new(cc, options_guioxide)))
        }),
    );
    if let Some(dossier) = etat_capture {
        let _ = std::fs::remove_dir_all(dossier);
    }
    resultat
}
