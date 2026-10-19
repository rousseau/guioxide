//! Écrans des onglets Examen et Reconstruction : couches lues dans les dérivées de la session, vues liées par le
//! curseur monde.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{RichText, Ui};
use guioxide_bids::{CheminsSession, Session};
use guioxide_data::{Chargeur, Etat, TableEtiquettes, VolumeLu};
use guioxide_geom::PointMonde;
use guioxide_view::coupe::{Couche, Rendu, Triplanaire, TypeVue, VueCoupe};
use guioxide_view::theme;

/// Taille du cache des volumes (octets de voxels).
const CACHE: usize = 4 << 30;

/// Opacité initiale de la segmentation.
const OPACITE: f32 = 0.45;

/// Couleur du contour des masques.
const CONTOUR: [u8; 3] = [0xE0, 0x7B, 0x39];

/// État d'un fichier de couche.
enum Statut {
    Absent,
    EnCours,
    Pret(Arc<VolumeLu>),
    Erreur(String),
}

impl Statut {
    fn pret(&self) -> Option<&Arc<VolumeLu>> {
        match self {
            Statut::Pret(v) => Some(v),
            _ => None,
        }
    }
}

struct Reconstruction {
    triplanaire: Triplanaire,
    fenetre: Option<(f32, f32)>,
    segmentation_visible: bool,
    opacite: f32,
    etiquettes: Option<Result<TableEtiquettes, String>>,
}

struct Examen {
    pile: usize,
    vue: Option<VueCoupe>,
    fenetre: Option<(f32, f32)>,
    masque_visible: bool,
}

pub struct Visualisation {
    chargeur: Chargeur,
    /// Session affichée (dossier des dérivées, ligne du tableau) : un changement remet les écrans à zéro.
    session: Option<(PathBuf, usize)>,
    curseur_place: bool,
    reconstruction: Reconstruction,
    examen: Examen,
}

impl Visualisation {
    pub fn new(ctx: &egui::Context) -> Visualisation {
        let ctx = ctx.clone();
        Visualisation {
            chargeur: Chargeur::new(CACHE, move || ctx.request_repaint()),
            session: None,
            curseur_place: false,
            reconstruction: Reconstruction::new(),
            examen: Examen::new(),
        }
    }

    /// À appeler à chaque image avec la session choisie : remet les écrans à zéro si elle a changé.
    pub fn suivre(&mut self, derivees: &Path, ligne: usize) {
        let cle = Some((derivees.to_path_buf(), ligne));
        if self.session != cle {
            self.session = cle;
            self.curseur_place = false;
            self.reconstruction = Reconstruction::new();
            self.examen = Examen::new();
        }
    }

    fn statut(&mut self, chemin: &Path) -> Statut {
        if !chemin.exists() {
            return Statut::Absent;
        }
        match self.chargeur.demander(chemin) {
            Etat::EnCours => Statut::EnCours,
            Etat::Pret(v) => Statut::Pret(v),
            Etat::Erreur(e) => Statut::Erreur(e),
        }
    }

    /// Place le curseur au centre du premier volume affiché d'une session.
    fn placer_curseur(&mut self, volume: &VolumeLu, curseur: &mut PointMonde) {
        if !self.curseur_place {
            *curseur = volume.volume.centre();
            self.curseur_place = true;
        }
    }

    pub fn reconstruction(&mut self, ui: &mut Ui, chemins: &CheminsSession, curseur: &mut PointMonde) {
        let (svr, seg) = (self.statut(&chemins.svr()), self.statut(&chemins.segmentation()));
        if self.reconstruction.etiquettes.is_none() {
            let tsv = chemins.etiquettes();
            self.reconstruction.etiquettes = Some(if tsv.exists() {
                TableEtiquettes::lire(&tsv).map_err(|e| e.to_string())
            } else {
                Ok(TableEtiquettes::default())
            });
        }
        if let Some(v) = svr.pret().or(seg.pret()) {
            self.placer_curseur(&v.clone(), curseur);
        }
        let r = &mut self.reconstruction;
        let table = r
            .etiquettes
            .as_ref()
            .and_then(|t| t.as_ref().ok())
            .cloned()
            .unwrap_or_default();
        let mut couches = Vec::new();
        if let Some(v) = svr.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Image { fenetre: r.fenetre },
                visible: true,
            });
        }
        if let Some(v) = seg.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Etiquettes {
                    table: table.clone(),
                    opacite: r.opacite,
                },
                visible: r.segmentation_visible,
            });
        }

        egui::Panel::left("reco_couches")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong("Couches");
                    ligne_couche(ui, "Volume reconstruit", "medx-svr", &svr);
                    ligne_couche(ui, "Segmentation", "medx-bounti", &seg);
                    if seg.pret().is_some() {
                        ui.checkbox(&mut r.segmentation_visible, "Afficher la segmentation");
                        ui.add(egui::Slider::new(&mut r.opacite, 0.0..=1.0).text("opacité"));
                    }
                    if svr.pret().is_some() && ui.button("Fenêtrage automatique").clicked() {
                        r.fenetre = None;
                    }
                    if let Some(Err(e)) = &r.etiquettes {
                        ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
                    }
                    if let Some(v) = seg.pret() {
                        ui.add_space(8.0);
                        ui.strong("Étiquettes");
                        liste_etiquettes(ui, &table, &v.volume);
                    }
                });
            });
        egui::Panel::right("reco_curseur")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                panneau_curseur(ui, curseur, &[("volume", svr.pret()), ("segmentation", seg.pret())]);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if couches.is_empty() {
                message_central(ui, &[("Volume reconstruit", &svr), ("Segmentation", &seg)]);
                return;
            }
            r.triplanaire.ui(ui, &mut couches, curseur, |ui, case| {
                ui.painter().rect_filled(case, 0.0, theme::FOND);
                ui.painter().text(
                    case.center(),
                    egui::Align2::CENTER_CENTER,
                    "Courbe de mouvement (jalon 4)",
                    egui::FontId::proportional(13.0),
                    theme::TEXTE_SECONDAIRE,
                );
            });
        });
        // Le fenêtrage a pu changer dans les vues (clic droit glissé).
        if let Some(Couche {
            rendu: Rendu::Image { fenetre },
            ..
        }) = couches.first()
        {
            r.fenetre = *fenetre;
        }
    }

    pub fn examen(&mut self, ui: &mut Ui, session: &Session, chemins: &CheminsSession, curseur: &mut PointMonde) {
        if session.piles.is_empty() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Aucune pile dans anat/").color(theme::TEXTE_SECONDAIRE));
                });
            });
            return;
        }
        let n = self.examen.pile.min(session.piles.len() - 1);
        let pile = &session.piles[n];
        let (image, masque) = (self.statut(&pile.chemin), self.statut(&chemins.masque(pile)));
        if let Some(v) = image.pret() {
            self.placer_curseur(&v.clone(), curseur);
        }
        let e = &mut self.examen;
        let mut couches = Vec::new();
        if let Some(v) = image.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Image { fenetre: e.fenetre },
                visible: true,
            });
            if e.vue.is_none() {
                e.vue = Some(VueCoupe::new(TypeVue::Native(*v.volume.affine())));
            }
        }
        if let Some(v) = masque.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Contour { couleur: CONTOUR },
                visible: e.masque_visible,
            });
        }

        egui::Panel::left("examen_piles")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong(format!("Piles ({})", session.piles.len()));
                    for (i, p) in session.piles.iter().enumerate() {
                        if ui.selectable_label(i == n, RichText::new(&p.nom).monospace()).clicked() && i != n {
                            e.pile = i;
                            (e.vue, e.fenetre) = (None, None);
                        }
                    }
                    ui.add_space(8.0);
                    ligne_couche(ui, "Pile", "sourcedata", &image);
                    ligne_couche(ui, "Masque", "medx-fetalbet", &masque);
                    if masque.pret().is_some() {
                        ui.checkbox(&mut e.masque_visible, "Contour du masque");
                    }
                });
            });
        egui::Panel::right("examen_curseur")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                panneau_curseur(ui, curseur, &[("pile", image.pret()), ("masque", masque.pret())]);
            });
        egui::CentralPanel::default().show(ui, |ui| match &mut e.vue {
            Some(vue) if !couches.is_empty() => {
                let rect = ui.available_rect_before_wrap();
                vue.ui(ui, rect, &mut couches, curseur);
            }
            _ => message_central(ui, &[("Pile", &image)]),
        });
        if let Some(Couche {
            rendu: Rendu::Image { fenetre },
            ..
        }) = couches.first()
        {
            e.fenetre = *fenetre;
        }
    }
}

impl Reconstruction {
    fn new() -> Self {
        Reconstruction {
            triplanaire: Triplanaire::default(),
            fenetre: None,
            segmentation_visible: true,
            opacite: OPACITE,
            etiquettes: None,
        }
    }
}

impl Examen {
    fn new() -> Self {
        Examen {
            pile: 0,
            vue: None,
            fenetre: None,
            masque_visible: true,
        }
    }
}

/// Une couche dans la liste : nom, pipeline, état ; grille et source de l'affine une fois lue.
fn ligne_couche(ui: &mut Ui, nom: &str, pipeline: &str, statut: &Statut) {
    ui.horizontal(|ui| {
        match statut {
            Statut::Absent => ui.label(RichText::new("○").color(theme::TEXTE_SECONDAIRE)),
            Statut::EnCours => ui.spinner(),
            Statut::Pret(_) => ui.label(RichText::new("✓").color(theme::TERMINE)),
            Statut::Erreur(_) => ui.label(RichText::new("⚠").color(theme::ERREUR)),
        };
        ui.label(nom);
        ui.label(
            RichText::new(pipeline)
                .monospace()
                .small()
                .color(theme::TEXTE_SECONDAIRE),
        );
    });
    let detail = match statut {
        Statut::Absent => Some(("absent".to_owned(), theme::TEXTE_SECONDAIRE)),
        Statut::EnCours => Some(("lecture…".to_owned(), theme::TEXTE_SECONDAIRE)),
        Statut::Erreur(e) => Some((e.clone(), theme::ERREUR)),
        Statut::Pret(v) => {
            let [nx, ny, nz] = v.volume.dims();
            let t = v.volume.taille_voxel().map(|x| format!("{x:.2}").replace('.', ","));
            Some((
                format!(
                    "{nx} × {ny} × {nz} · {} × {} × {} mm · {}",
                    t[0], t[1], t[2], v.source_affine
                ),
                theme::TEXTE_SECONDAIRE,
            ))
        }
    };
    if let Some((texte, couleur)) = detail {
        ui.label(RichText::new(texte).small().color(couleur));
    }
}

/// Étiquettes présentes dans la table ou dans le volume (indices distincts, fond exclu).
fn liste_etiquettes(ui: &mut Ui, table: &TableEtiquettes, volume: &guioxide_geom::Volume) {
    let mut indices: std::collections::BTreeSet<u32> = table.etiquettes.keys().copied().filter(|&i| i > 0).collect();
    if indices.is_empty() {
        indices = volume
            .voxels()
            .iter()
            .filter(|v| **v >= 1.0)
            .map(|v| v.round() as u32)
            .collect();
    }
    egui::Grid::new("etiquettes")
        .num_columns(3)
        .spacing([6.0, 2.0])
        .show(ui, |ui| {
            for i in indices {
                let [r, g, b] = table.couleur(i);
                let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                ui.label(RichText::new(i.to_string()).monospace());
                let nom = table.etiquettes.get(&i).map(|e| e.nom.clone()).unwrap_or_default();
                ui.label(nom);
                ui.end_row();
            }
        });
}

/// Position du curseur et valeur de chaque couche à cet endroit (indices de voxel arrondis).
fn panneau_curseur(ui: &mut Ui, curseur: &PointMonde, couches: &[(&str, Option<&Arc<VolumeLu>>)]) {
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

fn message_central(ui: &mut Ui, couches: &[(&str, &Statut)]) {
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
