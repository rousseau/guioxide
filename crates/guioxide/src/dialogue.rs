//! Fenêtres de choix de fichier ou de dossier, sans bloquer la boucle d'egui.
//!
//! `rfd::FileDialog` (synchrone) lance sous macOS une boucle modale imbriquée depuis le rappel de winit, qui panique
//! (« tried to handle event while another event is currently being handled »). La version asynchrone ouvre une
//! feuille attachée à la fenêtre et rend la main tout de suite : le futur est créé ici, sur le fil principal, puis
//! attendu dans un autre fil, comme un [`crate::ouverture::Chargement`].

use std::future::Future;
use std::path::PathBuf;
use std::sync::mpsc;

/// Fenêtre de choix ouverte ; son résultat arrive par [`Dialogue::resultat`] (un chemin, ou plusieurs pour
/// [`Dialogue::fichiers`]).
pub struct Dialogue<T = PathBuf> {
    recepteur: mpsc::Receiver<Option<T>>,
}

impl Dialogue<Vec<PathBuf>> {
    /// Choix de plusieurs fichiers, filtrés par extension (`nii`, `gz`…).
    pub fn fichiers(ctx: &egui::Context, titre: &str, filtre: &str, extensions: &[&str]) -> Self {
        let futur = rfd::AsyncFileDialog::new()
            .set_title(titre)
            .add_filter(filtre, extensions)
            .pick_files();
        lancer(ctx, async move {
            futur.await.map(|f| f.iter().map(|f| f.path().to_path_buf()).collect())
        })
    }
}

impl Dialogue {
    /// Choix d'un dossier.
    pub fn dossier(ctx: &egui::Context, titre: &str) -> Dialogue {
        let futur = rfd::AsyncFileDialog::new().set_title(titre).pick_folder();
        lancer(ctx, async { futur.await.map(chemin) })
    }

    /// Choix d'un fichier à écrire.
    pub fn enregistrer(ctx: &egui::Context, fenetre: rfd::AsyncFileDialog) -> Dialogue {
        let futur = fenetre.save_file();
        lancer(ctx, async { futur.await.map(chemin) })
    }
}

impl<T> Dialogue<T> {
    /// `None` tant que la fenêtre est ouverte ; `Some(None)` si elle a été annulée.
    pub fn resultat(&self) -> Option<Option<T>> {
        match self.recepteur.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(None),
        }
    }
}

fn chemin(f: rfd::FileHandle) -> PathBuf {
    f.path().to_path_buf()
}

/// Le futur est créé par l'appelant, sur le fil principal (la fenêtre s'ouvre alors), puis attendu ailleurs.
fn lancer<T: Send + 'static>(
    ctx: &egui::Context,
    futur: impl Future<Output = Option<T>> + Send + 'static,
) -> Dialogue<T> {
    let (emetteur, recepteur) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = emetteur.send(pollster::block_on(futur));
        ctx.request_repaint();
    });
    Dialogue { recepteur }
}
