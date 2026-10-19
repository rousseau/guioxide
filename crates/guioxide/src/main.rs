//! guioxide : interface graphique de la suite oxide pour l'IRM fœtale.
//!
//! `guioxide [chemin]` : ouvre la fenêtre, et le jeu BIDS indiqué s'il y en a un.

mod app;
mod etapes;
mod examen;
mod ouverture;
mod reconstruction;
mod simulation;
mod surfaces;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let jeu = std::env::args_os().nth(1).map(PathBuf::from);
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
            Ok(Box::new(app::Application::new(cc, jeu)))
        }),
    )
}
