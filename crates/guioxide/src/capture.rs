//! Capture de la fenêtre en PNG : raccourci `Ctrl/Cmd + Maj + S`, ou `--capture` en ligne de commande (capture
//! une fois les données affichées, puis sortie).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::ColorImage;

/// Durée sans lecture en cours avant une capture automatique : le temps que les vues se composent.
const STABLE: Duration = Duration::from_millis(1500);

/// Délai maximal d'une capture `--capture` : au-delà, la fenêtre ne se redessine sans doute pas (écran verrouillé
/// ou en veille, fenêtre masquée) et la boucle d'egui ne rend plus la main.
const DELAI_MAX: Duration = Duration::from_secs(300);

/// Surveille une capture `--capture` depuis un autre fil : quitte en erreur si elle n'est pas écrite à temps.
pub fn surveiller(chemin: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(DELAI_MAX);
        eprintln!(
            "capture non écrite après {} s : {} (écran verrouillé ou en veille, fenêtre masquée ?)",
            DELAI_MAX.as_secs(),
            chemin.display()
        );
        std::process::exit(1);
    });
}

/// Écrit une image egui (RVBA, sRGB) en PNG.
pub fn enregistrer_png(image: &ColorImage, chemin: &Path) -> Result<(), String> {
    let fichier = std::fs::File::create(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
    let [l, h] = image.size;
    let mut encodeur = png::Encoder::new(std::io::BufWriter::new(fichier), l as u32, h as u32);
    encodeur.set_color(png::ColorType::Rgba);
    encodeur.set_depth(png::BitDepth::Eight);
    let mut ecrivain = encodeur
        .write_header()
        .map_err(|e| format!("{} : {e}", chemin.display()))?;
    let octets: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    ecrivain
        .write_image_data(&octets)
        .map_err(|e| format!("{} : {e}", chemin.display()))
}

/// Une capture demandée ; le résultat arrive d'un autre fil (rappel d'egui).
pub struct Capture {
    pub chemin: PathBuf,
    /// Fermer l'application une fois l'image écrite (`--capture`).
    pub quitter: bool,
    /// Capture automatique : attend que rien ne soit en cours de lecture depuis [`STABLE`].
    pub attendre: bool,
    stable_depuis: Option<Instant>,
    demandee: bool,
    pub resultat: Arc<Mutex<Option<Result<PathBuf, String>>>>,
}

impl Capture {
    pub fn new(chemin: PathBuf, quitter: bool, attendre: bool) -> Capture {
        Capture {
            chemin,
            quitter,
            attendre,
            stable_depuis: None,
            demandee: false,
            resultat: Default::default(),
        }
    }

    /// À chaque image : demande la capture quand c'est le moment (`occupe` : une lecture était en cours).
    pub fn suivre(&mut self, ctx: &egui::Context, occupe: bool) {
        if self.demandee {
            return;
        }
        if self.attendre {
            if occupe {
                self.stable_depuis = None;
            }
            let depuis = *self.stable_depuis.get_or_insert_with(Instant::now);
            if depuis.elapsed() < STABLE {
                ctx.request_repaint_after(Duration::from_millis(100));
                return;
            }
        }
        self.demandee = true;
        let (chemin, resultat, ctx2) = (self.chemin.clone(), self.resultat.clone(), ctx.clone());
        ctx.request_screenshot(move |image| {
            let r = enregistrer_png(&image, &chemin).map(|()| chemin);
            if let Ok(mut place) = resultat.lock() {
                *place = Some(r);
            }
            ctx2.request_repaint();
        });
    }

    /// Résultat de l'écriture, s'il est arrivé.
    pub fn resultat(&self) -> Option<Result<PathBuf, String>> {
        self.resultat.lock().ok().and_then(|mut r| r.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_lisible() {
        let dir = tempfile::tempdir().unwrap();
        let chemin = dir.path().join("c.png");
        let image = ColorImage::new([3, 2], vec![egui::Color32::from_rgb(10, 20, 30); 6]);
        enregistrer_png(&image, &chemin).unwrap();
        let decodeur = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&chemin).unwrap()));
        let mut lecteur = decodeur.read_info().unwrap();
        let mut tampon = vec![0; lecteur.output_buffer_size().unwrap()];
        let info = lecteur.next_frame(&mut tampon).unwrap();
        assert_eq!((info.width, info.height), (3, 2));
        assert_eq!(&tampon[..4], &[10, 20, 30, 255]);
    }
}
