//! guioxide : interface graphique de la suite oxide pour l'IRM fœtale.
//!
//! ```text
//! guioxide [chemin du jeu BIDS] [--onglet 1-4] [--session <texte>] [--capture <fichier.png>]
//! ```
//!
//! - `--onglet` : ouvre directement une étape (1 Examen … 4 Simulation) ;
//! - `--session` : choisit la session dont le libellé contient ce texte (`sub-01 / ses-02`), la première sinon ;
//! - `--capture` : enregistre la fenêtre en PNG une fois les données affichées, puis quitte (erreur après 5 min si
//!   la fenêtre ne se redessine pas : écran verrouillé, fenêtre masquée).

mod app;
mod capture;
mod etapes;
mod examen;
mod ouverture;
mod reconstruction;
mod simulation;
mod surfaces;

use std::path::PathBuf;

use guioxide_bids::Etape;

const USAGE: &str = "usage : guioxide [chemin] [--onglet 1-4] [--session <texte>] [--capture <fichier.png>]";

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
            Some("--capture") => options.capture = Some(PathBuf::from(valeur("--capture")?)),
            Some("-h" | "--help") => return Err(USAGE.to_owned()),
            Some(o) if o.starts_with("--") => return Err(format!("option inconnue : {o}\n{USAGE}")),
            _ if options.jeu.is_none() => options.jeu = Some(PathBuf::from(a)),
            _ => return Err(format!("un seul jeu à la fois\n{USAGE}")),
        }
    }
    Ok(options)
}

fn main() -> eframe::Result {
    let options_guioxide = match options() {
        Ok(o) => o,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if let Some(chemin) = &options_guioxide.capture {
        capture::surveiller(chemin.clone());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("guioxide")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "guioxide",
        options,
        Box::new(|cc| {
            guioxide_view::theme::appliquer(&cc.egui_ctx);
            guioxide_view::polices::installer(&cc.egui_ctx);
            Ok(Box::new(app::Application::new(cc, options_guioxide)))
        }),
    )
}
