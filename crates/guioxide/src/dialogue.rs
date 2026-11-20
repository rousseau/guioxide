//! Fenêtres de choix de fichier ou de dossier, sans bloquer la boucle d'egui.
//!
//! `rfd::FileDialog` (synchrone) lance sous macOS une boucle modale imbriquée depuis le rappel de winit, qui panique
//! (« tried to handle event while another event is currently being handled »). La version asynchrone ouvre une
//! feuille attachée à la fenêtre et rend la main tout de suite : le futur est créé ici, sur le fil principal, puis
//! attendu dans un autre fil, comme un [`crate::ouverture::Chargement`].

use std::future::Future;
use std::path::PathBuf;
use std::sync::mpsc;

/// Fenêtre de choix ouverte ; son résultat arrive par [`Dialogue::resultat`].
pub struct Dialogue {
    recepteur: mpsc::Receiver<Option<PathBuf>>,
}

impl Dialogue {
    /// Choix d'un dossier.
    pub fn dossier(ctx: &egui::Context, titre: &str) -> Dialogue {
        Dialogue::lancer(ctx, rfd::AsyncFileDialog::new().set_title(titre).pick_folder())
    }

    /// Choix d'un fichier à écrire.
    pub fn enregistrer(ctx: &egui::Context, fenetre: rfd::AsyncFileDialog) -> Dialogue {
        Dialogue::lancer(ctx, fenetre.save_file())
    }

    fn lancer(ctx: &egui::Context, futur: impl Future<Output = Option<rfd::FileHandle>> + Send + 'static) -> Dialogue {
        let (emetteur, recepteur) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let chemin = pollster::block_on(futur).map(|f| f.path().to_path_buf());
            let _ = emetteur.send(chemin);
            ctx.request_repaint();
        });
        Dialogue { recepteur }
    }

    /// `None` tant que la fenêtre est ouverte ; `Some(None)` si elle a été annulée.
    pub fn resultat(&self) -> Option<Option<PathBuf>> {
        match self.recepteur.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(None),
        }
    }
}
