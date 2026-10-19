//! Base commune des onglets Examen et Reconstruction : lecture des volumes en arrière-plan, données de la session
//! (qualité, mouvement, paramètres, géométrie des piles) lues une fois, panneaux partagés.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{RichText, Ui};
use guioxide_bids::{CheminsSession, Session};
use guioxide_data::{
    Chargeur, ErreurDonnees, Etat, Maillage, MouvementCoupe, QualiteCoupe, VolumeLu, lire_geometrie, lire_mouvement,
    lire_qualite,
};
use guioxide_geom::{Affine, PointMonde};
use guioxide_view::theme;

use crate::examen::Examen;
use crate::reconstruction::Reconstruction;
use crate::simulation::Simulation;
use crate::surfaces::Surfaces;

/// Taille du cache des volumes (octets de voxels).
const CACHE: usize = 4 << 30;

/// Couleur du contour des masques.
pub const CONTOUR: [u8; 3] = [0xE0, 0x7B, 0x39];

/// État d'un volume de couche.
pub enum Statut<T = VolumeLu> {
    Absent,
    EnCours,
    Pret(Arc<T>),
    Erreur(String),
}

impl<T> Statut<T> {
    pub fn pret(&self) -> Option<&Arc<T>> {
        match self {
            Statut::Pret(v) => Some(v),
            _ => None,
        }
    }
}

/// Un petit fichier du contrat, lu une fois par session.
pub enum Fichier<T> {
    Absent,
    Lu(T),
    Erreur(String),
}

impl<T> Fichier<T> {
    fn lire(chemin: &Path, lire: impl FnOnce(&Path) -> Result<T, ErreurDonnees>) -> Fichier<T> {
        if !chemin.exists() {
            return Fichier::Absent;
        }
        match lire(chemin) {
            Ok(v) => Fichier::Lu(v),
            Err(e) => Fichier::Erreur(e.to_string()),
        }
    }

    pub fn lu(&self) -> Option<&T> {
        match self {
            Fichier::Lu(v) => Some(v),
            _ => None,
        }
    }

    /// Message d'erreur, à afficher à côté de ce qui en dépend.
    pub fn erreur(&self) -> Option<&str> {
        match self {
            Fichier::Erreur(e) => Some(e),
            _ => None,
        }
    }
}

/// Fichiers de la session hors volumes : petits, lus une fois quand la session change.
pub struct DonneesSession {
    pub qualite: Fichier<Vec<QualiteCoupe>>,
    pub mouvement: Fichier<Vec<MouvementCoupe>>,
    pub parametres: Fichier<serde_json::Value>,
    /// Géométrie de chaque pile (dimensions, affine), lue dans l'en-tête seul, par nom de pile.
    pub geometries: HashMap<String, Result<([usize; 3], Affine), String>>,
}

impl DonneesSession {
    fn lire(session: &Session, chemins: &CheminsSession) -> DonneesSession {
        let json = |c: &Path| -> Result<serde_json::Value, ErreurDonnees> {
            let texte = std::fs::read_to_string(c).map_err(|e| ErreurDonnees::Lecture {
                chemin: c.into(),
                source: e,
            })?;
            serde_json::from_str(&texte).map_err(|e| ErreurDonnees::Format {
                chemin: c.into(),
                message: e.to_string(),
            })
        };
        DonneesSession {
            qualite: Fichier::lire(&chemins.qualite(), lire_qualite),
            mouvement: Fichier::lire(&chemins.mouvement(), lire_mouvement),
            parametres: Fichier::lire(&chemins.parametres_svr(), json),
            geometries: session
                .piles
                .iter()
                .map(|p| (p.nom.clone(), lire_geometrie(&p.chemin).map_err(|e| e.to_string())))
                .collect(),
        }
    }

    /// Lignes de qualité d'une pile.
    pub fn qualite_pile(&self, pile: &str) -> Vec<&QualiteCoupe> {
        self.qualite
            .lu()
            .map(|q| q.iter().filter(|l| l.stack == pile).collect())
            .unwrap_or_default()
    }
}

/// Lecture des volumes et curseur initial, partagés par les écrans.
pub struct Base {
    volumes: Chargeur<VolumeLu>,
    surfaces: Chargeur<Maillage>,
    champs: Chargeur<Vec<f32>>,
    curseur_place: bool,
}

fn statut<T: Send + Sync + 'static>(chargeur: &mut Chargeur<T>, chemin: &Path) -> Statut<T> {
    if !chemin.exists() {
        return Statut::Absent;
    }
    match chargeur.demander(chemin) {
        Etat::EnCours => Statut::EnCours,
        Etat::Pret(v) => Statut::Pret(v),
        Etat::Erreur(e) => Statut::Erreur(e),
    }
}

impl Base {
    pub fn statut(&mut self, chemin: &Path) -> Statut {
        statut(&mut self.volumes, chemin)
    }

    pub fn surface(&mut self, chemin: &Path) -> Statut<Maillage> {
        statut(&mut self.surfaces, chemin)
    }

    pub fn champ(&mut self, chemin: &Path) -> Statut<Vec<f32>> {
        statut(&mut self.champs, chemin)
    }

    /// Place le curseur au centre du premier volume affiché d'une session.
    pub fn placer_curseur(&mut self, volume: &VolumeLu, curseur: &mut PointMonde) {
        if !self.curseur_place {
            *curseur = volume.volume.centre();
            self.curseur_place = true;
        }
    }
}

pub struct Visualisation {
    base: Base,
    /// Session affichée (dossier des dérivées, ligne du tableau) : un changement remet les écrans à zéro.
    session: Option<(PathBuf, usize)>,
    donnees: Option<DonneesSession>,
    examen: Examen,
    reconstruction: Reconstruction,
    surfaces: Surfaces,
    simulation: Simulation,
}

impl Visualisation {
    pub fn new(ctx: &egui::Context) -> Visualisation {
        let reveil = |ctx: &egui::Context| {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        };
        Visualisation {
            base: Base {
                volumes: Chargeur::volumes(CACHE, reveil(ctx)),
                surfaces: Chargeur::surfaces(CACHE / 4, reveil(ctx)),
                champs: Chargeur::champs(CACHE / 16, reveil(ctx)),
                curseur_place: false,
            },
            session: None,
            donnees: None,
            examen: Examen::default(),
            reconstruction: Reconstruction::default(),
            surfaces: Surfaces::default(),
            simulation: Simulation::default(),
        }
    }

    /// À appeler à chaque image avec la session choisie : remet les écrans à zéro si elle a changé.
    pub fn suivre(&mut self, derivees: &Path, ligne: usize) {
        let cle = Some((derivees.to_path_buf(), ligne));
        if self.session != cle {
            self.session = cle;
            self.base.curseur_place = false;
            self.donnees = None;
            self.examen = Examen::default();
            self.reconstruction = Reconstruction::default();
            self.surfaces = Surfaces::default();
            self.simulation = Simulation::default();
        }
    }

    pub fn examen(&mut self, ui: &mut Ui, session: &Session, chemins: &CheminsSession, curseur: &mut PointMonde) {
        let donnees = self
            .donnees
            .get_or_insert_with(|| DonneesSession::lire(session, chemins));
        self.examen.ui(ui, &mut self.base, donnees, session, chemins, curseur);
    }

    pub fn reconstruction(
        &mut self,
        ui: &mut Ui,
        session: &Session,
        chemins: &CheminsSession,
        curseur: &mut PointMonde,
    ) {
        let donnees = self
            .donnees
            .get_or_insert_with(|| DonneesSession::lire(session, chemins));
        self.reconstruction
            .ui(ui, &mut self.base, donnees, session, chemins, curseur);
    }
    pub fn surfaces(
        &mut self,
        ui: &mut Ui,
        chemins: &CheminsSession,
        rendu: Option<&egui_wgpu::RenderState>,
        curseur: &mut PointMonde,
    ) {
        self.surfaces.ui(ui, &mut self.base, chemins, rendu, curseur);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn simulation(
        &mut self,
        ui: &mut Ui,
        jeu: &guioxide_bids::JeuBids,
        (sujet, session): (usize, usize),
        derivees: &Path,
        chemins: &CheminsSession,
        rendu: Option<&egui_wgpu::RenderState>,
        curseur: &mut PointMonde,
    ) {
        self.simulation.ui(
            ui,
            &mut self.base,
            jeu,
            sujet,
            session,
            derivees,
            chemins,
            rendu,
            curseur,
        );
    }
}

/// Pastille d'état d'un fichier ou d'une couche.
pub fn icone_statut<T>(ui: &mut Ui, statut: &Statut<T>) {
    match statut {
        Statut::Absent => ui.label(RichText::new("○").color(theme::TEXTE_SECONDAIRE)),
        Statut::EnCours => ui.spinner(),
        Statut::Pret(_) => ui.label(RichText::new("✓").color(theme::TERMINE)),
        Statut::Erreur(_) => ui.label(RichText::new("⚠").color(theme::ERREUR)),
    };
}

/// Une couche dans la liste : nom, pipeline, état ; grille et source de l'affine une fois lue.
pub fn ligne_couche(ui: &mut Ui, nom: &str, pipeline: &str, statut: &Statut) {
    ui.horizontal(|ui| {
        icone_statut(ui, statut);
        ui.label(nom);
        ui.label(
            RichText::new(pipeline)
                .monospace()
                .small()
                .color(theme::TEXTE_SECONDAIRE),
        );
    });
    let (texte, couleur) = match statut {
        Statut::Absent => ("absent".to_owned(), theme::TEXTE_SECONDAIRE),
        Statut::EnCours => ("lecture…".to_owned(), theme::TEXTE_SECONDAIRE),
        Statut::Erreur(e) => (e.clone(), theme::ERREUR),
        Statut::Pret(v) => {
            let [nx, ny, nz] = v.volume.dims();
            let t = v.volume.taille_voxel().map(decimal2);
            (
                format!(
                    "{nx} × {ny} × {nz} · {} × {} × {} mm · {}",
                    t[0], t[1], t[2], v.source_affine
                ),
                theme::TEXTE_SECONDAIRE,
            )
        }
    };
    ui.label(RichText::new(texte).small().color(couleur));
}

/// Position du curseur et valeur de chaque couche à cet endroit (indices de voxel arrondis).
pub fn panneau_curseur(ui: &mut Ui, curseur: &PointMonde, couches: &[(&str, Option<&Arc<VolumeLu>>)]) {
    ui.strong("Curseur");
    let [x, y, z] = curseur.0.map(|v| format!("{v:.1}").replace('.', ","));
    ui.label(RichText::new(format!("{x}  {y}  {z} mm (RAS)")).monospace());
    ui.add_space(6.0);
    egui::Grid::new("valeurs_curseur").num_columns(3).show(ui, |ui| {
        for (nom, volume) in couches {
            let Some(v) = volume else { continue };
            let ijk = v.volume.vers_voxel(curseur.0).map(|c| c.round() as i64);
            let valeur = v
                .volume
                .plus_proche(curseur.0)
                .map_or("hors champ".to_owned(), |x| format!("{x:.1}"));
            ui.label(*nom);
            ui.label(
                RichText::new(format!("{} {} {}", ijk[0], ijk[1], ijk[2]))
                    .monospace()
                    .color(theme::TEXTE_SECONDAIRE),
            );
            ui.label(RichText::new(valeur).monospace());
            ui.end_row();
        }
    });
}

pub fn message_central(ui: &mut Ui, couches: &[(&str, &Statut)]) {
    ui.centered_and_justified(|ui| {
        let texte = if couches.iter().any(|(_, s)| matches!(s, Statut::EnCours)) {
            "Lecture des volumes…".to_owned()
        } else {
            let absents: Vec<&str> = couches
                .iter()
                .filter(|(_, s)| matches!(s, Statut::Absent))
                .map(|(n, _)| *n)
                .collect();
            format!(
                "Rien à afficher : {} absent(s) dans la sortie choisie",
                absents.join(", ")
            )
        };
        ui.label(RichText::new(texte).color(theme::TEXTE_SECONDAIRE));
    });
}

/// Bouton de validation : l'enregistrement d'une validation n'est pas encore dans le contrat.
pub fn bouton_valider(ui: &mut Ui) {
    ui.add_enabled(false, egui::Button::new("Valider →"))
        .on_disabled_hover_text("Où enregistrer la validation et la note de l'opérateur : à définir dans le contrat");
}

/// Nombre à deux décimales, virgule décimale.
pub fn decimal2(x: f64) -> String {
    format!("{x:.2}").replace('.', ",")
}

/// Nom court d'une pile : sans le préfixe `sub-X_ses-Y_` ni le suffixe de contraste.
pub fn nom_court<'a>(nom: &'a str, prefixe: &str) -> &'a str {
    let sans = nom
        .strip_prefix(prefixe)
        .and_then(|n| n.strip_prefix('_'))
        .unwrap_or(nom);
    sans.rsplit_once('_').map_or(sans, |(debut, _)| debut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_courts_des_piles() {
        assert_eq!(
            nom_court("sub-01_ses-02_acq-trufiax_run-1_T2w", "sub-01_ses-02"),
            "acq-trufiax_run-1"
        );
        assert_eq!(nom_court("sub-01_T2w", "sub-01"), "T2w");
        assert_eq!(nom_court("autre_T2w", "sub-01"), "autre");
    }
}
