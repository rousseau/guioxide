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

/// Attente maximale du redimensionnement de la fenêtre à la taille de capture.
const ATTENTE_TAILLE: Duration = Duration::from_secs(5);

/// Taille de la fenêtre à l'ouverture, en points ; aussi la taille en pixels des captures `--capture`, quelle que soit
/// l'échelle de l'écran (voir [`Capture::a_l_echelle_un`]).
pub const TAILLE_FENETRE: [f32; 2] = [1400.0, 900.0];

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
    /// Capture à un pixel par point, fenêtre de cette taille en pixels ([`Capture::a_l_echelle_un`]).
    echelle_un: Option<egui::Vec2>,
    /// Moment où la taille en pixels de la fenêtre a été fixée (demandée si l'écran n'est pas à l'échelle 1).
    redimension: Option<Instant>,
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
            echelle_un: None,
            redimension: None,
            stable_depuis: None,
            demandee: false,
            resultat: Default::default(),
        }
    }

    /// Capture indépendante de l'écran (`--capture`, tests visuels) : un pixel par point, et une fenêtre de `taille`
    /// pixels. Sur un écran à l'échelle 2 (Retina), la fenêtre ouverte à 1 400 × 900 points ferait 2 800 × 1 800 pixels ;
    /// elle est ramenée à 1 400 × 900 pixels, avec le même rendu que sur un écran à l'échelle 1.
    pub fn a_l_echelle_un(mut self, taille: [f32; 2]) -> Capture {
        self.echelle_un = Some(taille.into());
        self
    }

    /// Vrai quand la fenêtre est à son échelle et à sa taille de capture ([`Capture::a_l_echelle_un`]) ; sinon, fait le
    /// pas suivant. Le zoom ne s'applique qu'à l'image suivante : d'abord l'échelle, puis la taille en pixels, puis
    /// l'attente du redimensionnement (au plus [`ATTENTE_TAILLE`], si la fenêtre ne peut pas prendre cette taille).
    pub fn echelle_prete(&mut self, ctx: &egui::Context) -> bool {
        let Some(taille) = self.echelle_un else { return true };
        if ctx.pixels_per_point() != 1.0 {
            ctx.set_pixels_per_point(1.0);
            ctx.request_repaint();
            return false;
        }
        let depuis = *self.redimension.get_or_insert_with(|| {
            if ctx.native_pixels_per_point().is_some_and(|n| n != 1.0) {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(taille));
            }
            Instant::now()
        });
        if ctx.content_rect().size() != taille && depuis.elapsed() < ATTENTE_TAILLE {
            ctx.request_repaint_after(Duration::from_millis(50));
            return false;
        }
        true
    }

    /// À chaque image : demande la capture quand c'est le moment (`occupe` : une lecture était en cours).
    pub fn suivre(&mut self, ctx: &egui::Context, occupe: bool) {
        if self.demandee {
            return;
        }
        if !self.echelle_prete(ctx) {
            self.stable_depuis = None;
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
