//! Onglet 3 · Surfaces : maillages de surfoxide, contours dans trois coupes, vue 3D colorée par un champ par sommet,
//! contrôle des maillages.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Color32, RichText, Ui, Vec2};
use egui_wgpu::RenderState;
use guioxide_bids::{CheminsSession, Etape};
use guioxide_data::{Maillage, SurfQc, TableEtiquettes, lire_controle_surfaces};
use guioxide_geom::{Orientation, PointMonde};
use guioxide_view::coupe::{Couche, Rendu, Surface, TypeVue, VueCoupe};
use guioxide_view::palettes::{colorer, divergente, sequentielle};
use guioxide_view::theme;
use guioxide_view::vue3d::{Objet3d, Vue3d, VuePredefinie};

use crate::etapes::{Base, Statut, icone_statut, message_central, panneau_curseur};

/// Surfaces du contrat, dans l'ordre d'affichage, avec leur couleur par défaut.
const TYPES: [(&str, &str, [u8; 3]); 3] = [
    ("white", "blanche", [0x7F, 0xB3, 0xE6]),
    ("pial", "piale", [0xF2, 0xD0, 0x4B]),
    ("midthickness", "médiane", [0x7F, 0xE6, 0xA0]),
];
const HEMISPHERES: [Option<&str>; 3] = [Some("L"), Some("R"), None];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Superposition {
    Contours,
    Segmentation,
    Rien,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Coloration {
    Uni,
    Courbure,
    Profondeur,
    Epaisseur,
}

impl Coloration {
    /// Nom du champ (`<prefixe>_hemi-X_<champ>.shape.gii`) et palette divergente ou non.
    fn champ(self) -> Option<(&'static str, bool)> {
        match self {
            Coloration::Uni => None,
            Coloration::Courbure => Some(("curv", true)),
            Coloration::Profondeur => Some(("sulc", true)),
            Coloration::Epaisseur => Some(("thickness", false)),
        }
    }

    fn libelle(self) -> &'static str {
        match self {
            Coloration::Uni => "uni",
            Coloration::Courbure => "courbure",
            Coloration::Profondeur => "profondeur sulcale",
            Coloration::Epaisseur => "épaisseur (mm)",
        }
    }
}

/// Couleurs par sommet d'un maillage pour un champ, et bornes de la palette.
type Coloree = (Arc<Vec<[u8; 3]>>, (f32, f32));

/// Une surface du contrat : nom, hémisphère, affichage.
struct LigneSurface {
    nom: &'static str,
    libelle: &'static str,
    hemi: Option<&'static str>,
    couleur: [u8; 3],
    visible: bool,
}

pub struct Surfaces {
    lignes: Vec<LigneSurface>,
    superposition: Superposition,
    coloration: Coloration,
    vues: [VueCoupe; 3],
    vue3d: Vue3d,
    /// Hémisphère des vues prédéfinies.
    gauche: bool,
    fenetre: Option<(f32, f32)>,
    /// Couleurs par sommet, par (maillage, champ) : recalculées seulement quand l'un change.
    couleurs: BTreeMap<(usize, usize), Coloree>,
    unies: BTreeMap<[u8; 3], Arc<Vec<[u8; 3]>>>,
    controle: Option<Result<Vec<SurfQc>, String>>,
}

impl Default for Surfaces {
    fn default() -> Self {
        let mut lignes = Vec::new();
        for (nom, libelle, couleur) in TYPES {
            for hemi in HEMISPHERES {
                lignes.push(LigneSurface {
                    nom,
                    libelle,
                    hemi,
                    couleur,
                    visible: nom != "midthickness",
                });
            }
        }
        Surfaces {
            lignes,
            superposition: Superposition::Contours,
            coloration: Coloration::Uni,
            vues: Orientation::TOUTES.map(|o| VueCoupe::new(TypeVue::Alignee(o))),
            vue3d: Vue3d::default(),
            gauche: true,
            fenetre: None,
            couleurs: BTreeMap::new(),
            unies: BTreeMap::new(),
            controle: None,
        }
    }
}

fn libelle_hemi(hemi: Option<&str>) -> &'static str {
    match hemi {
        Some("L") => "gauche",
        Some("R") => "droite",
        _ => "deux hémisphères",
    }
}

impl Surfaces {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        base: &mut Base,
        chemins: &CheminsSession,
        rendu: Option<&RenderState>,
        curseur: &mut PointMonde,
    ) {
        // Fichiers : surfaces présentes, champs de la coloration, volume et segmentation sous-jacents.
        let statuts: Vec<Statut<Maillage>> = self
            .lignes
            .iter()
            .map(|l| base.surface(&chemins.surface(l.nom, l.hemi)))
            .collect();
        let presentes: Vec<usize> = (0..self.lignes.len())
            .filter(|&i| !matches!(statuts[i], Statut::Absent))
            .collect();
        let champs: BTreeMap<Option<&str>, Statut<Vec<f32>>> = match self.coloration.champ() {
            Some((nom, _)) => HEMISPHERES
                .iter()
                .map(|h| (*h, base.champ(&chemins.champ(nom, *h))))
                .collect(),
            None => BTreeMap::new(),
        };
        let svr = base.statut(&chemins.svr());
        let seg = if self.superposition == Superposition::Segmentation {
            base.statut(&chemins.segmentation())
        } else {
            Statut::Absent
        };
        if let Some(v) = svr.pret() {
            base.placer_curseur(v, curseur);
        }
        if self.controle.is_none() {
            let c = chemins.controle_surfaces();
            self.controle = c
                .exists()
                .then(|| lire_controle_surfaces(&c).map_err(|e| e.to_string()));
        }

        // Objets : contours 2D et maillages 3D colorés.
        let mut contours = Vec::new();
        let mut objets = Vec::new();
        let mut echelle = None;
        for (i, ligne) in self.lignes.iter().enumerate() {
            let Some(m) = statuts[i].pret() else { continue };
            let id = Arc::as_ptr(m) as usize;
            contours.push(Surface {
                maillage: m.clone(),
                couleur: ligne.couleur,
                visible: ligne.visible && self.superposition == Superposition::Contours,
            });
            let champ = champs
                .get(&ligne.hemi)
                .and_then(Statut::pret)
                .filter(|c| c.len() == m.sommets.len());
            let couleurs = match (champ, self.coloration.champ()) {
                (Some(c), Some((_, divergent))) => {
                    let cle = (id, Arc::as_ptr(c) as usize);
                    let (couleurs, bornes) = self
                        .couleurs
                        .entry(cle)
                        .or_insert_with(|| {
                            let (couleurs, bornes) = colorer(c, divergent);
                            (Arc::new(couleurs), bornes)
                        })
                        .clone();
                    echelle = Some((bornes, divergent));
                    couleurs
                }
                _ => self
                    .unies
                    .entry(ligne.couleur)
                    .or_insert_with(|| Arc::new(vec![ligne.couleur]))
                    .clone(),
            };
            objets.push(Objet3d {
                maillage: m.clone(),
                couleurs,
                visible: ligne.visible,
            });
        }
        let mut couches = Vec::new();
        if let Some(v) = svr.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Image { fenetre: self.fenetre },
                visible: true,
            });
        }
        if let Some(v) = seg.pret() {
            let table = TableEtiquettes::lire(&chemins.etiquettes()).unwrap_or_default();
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Etiquettes { table, opacite: 0.45 },
                visible: true,
            });
        }

        egui::Panel::left("surfaces_liste")
            .resizable(true)
            .default_size(270.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong("Maillages");
                    if presentes.is_empty() {
                        ui.label(
                            RichText::new("Aucune surface surfx-surfaces dans la sortie choisie")
                                .color(theme::TEXTE_SECONDAIRE),
                        );
                    }
                    for &i in &presentes {
                        let ligne = &mut self.lignes[i];
                        ui.horizontal(|ui| {
                            icone_statut(ui, &statuts[i]);
                            ui.checkbox(&mut ligne.visible, "");
                            let mut c = Color32::from_rgb(ligne.couleur[0], ligne.couleur[1], ligne.couleur[2]);
                            if ui.color_edit_button_srgba(&mut c).changed() {
                                ligne.couleur = [c.r(), c.g(), c.b()];
                            }
                            ui.label(format!("{} · {}", ligne.libelle, libelle_hemi(ligne.hemi)));
                        });
                        if let Statut::Erreur(e) = &statuts[i] {
                            ui.label(RichText::new(e).small().color(theme::ERREUR));
                        }
                    }
                    ui.add_space(8.0);
                    ui.strong("Superposition 2D");
                    ui.radio_value(
                        &mut self.superposition,
                        Superposition::Contours,
                        "contours des maillages",
                    );
                    ui.radio_value(
                        &mut self.superposition,
                        Superposition::Segmentation,
                        "segmentation source",
                    );
                    ui.radio_value(&mut self.superposition, Superposition::Rien, "rien");
                });
            });
        egui::Panel::right("surfaces_controle")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong("Coloration");
                    for c in [
                        Coloration::Uni,
                        Coloration::Courbure,
                        Coloration::Profondeur,
                        Coloration::Epaisseur,
                    ] {
                        ui.radio_value(&mut self.coloration, c, c.libelle());
                    }
                    if let Some((nom, _)) = self.coloration.champ() {
                        for (hemi, statut) in &champs {
                            if !matches!(statut, Statut::Absent) || hemi.is_some() {
                                ui.horizontal(|ui| {
                                    icone_statut(ui, statut);
                                    ui.label(RichText::new(format!("{nom} · {}", libelle_hemi(*hemi))).small());
                                });
                            }
                        }
                    }
                    if let Some(((bas, haut), divergent)) = echelle {
                        barre_de_couleur(ui, bas, haut, divergent);
                    }
                    ui.add_space(8.0);
                    ui.strong("Contrôle des maillages");
                    self.controle_ui(ui, &statuts);
                    ui.add_space(8.0);
                    panneau_curseur(ui, curseur, &[("volume", svr.pret())]);
                    ui.add_space(12.0);
                    base.valider(ui, Etape::Surfaces, chemins);
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if objets.is_empty() && couches.is_empty() {
                message_central(ui, &[("Volume reconstruit", &svr)]);
                return;
            }
            let zone = ui.available_rect_before_wrap();
            let hauteur_coupes = (zone.height() * 0.38).min(zone.width() / 3.0);
            let largeur = (zone.width() - 4.0) / 3.0;
            if !couches.is_empty() {
                for (k, vue) in self.vues.iter_mut().enumerate() {
                    let case = egui::Rect::from_min_size(
                        zone.min + Vec2::new(k as f32 * (largeur + 2.0), 0.0),
                        Vec2::new(largeur, hauteur_coupes),
                    );
                    vue.ui(ui, case, &mut couches, &contours, curseur);
                }
            }
            let haut = if couches.is_empty() {
                zone.top()
            } else {
                zone.top() + hauteur_coupes + 4.0
            };
            let barre = egui::Rect::from_min_max(egui::pos2(zone.left(), haut), egui::pos2(zone.right(), haut + 26.0));
            let mut enfant = ui.new_child(egui::UiBuilder::new().max_rect(barre));
            enfant.horizontal(|ui| {
                ui.selectable_value(&mut self.gauche, true, "gauche");
                ui.selectable_value(&mut self.gauche, false, "droite");
                ui.separator();
                for (vue, nom) in [
                    (VuePredefinie::Laterale, "latérale"),
                    (VuePredefinie::Mediale, "médiale"),
                    (VuePredefinie::Dorsale, "dorsale"),
                ] {
                    if ui.button(nom).clicked()
                        && let Some(c) = &mut self.vue3d.camera
                    {
                        c.regarder(vue, self.gauche);
                    }
                }
                if ui.button("recadrer").clicked() {
                    self.vue3d.recadrer();
                }
            });
            let rect3d = egui::Rect::from_min_max(egui::pos2(zone.left(), barre.bottom() + 2.0), zone.max);
            match rendu {
                Some(rs) => {
                    self.vue3d.ui(ui, rect3d, rs, &objets, curseur);
                }
                None => {
                    ui.painter().text(
                        rect3d.center(),
                        egui::Align2::CENTER_CENTER,
                        "Rendu 3D indisponible (pas de contexte wgpu)",
                        egui::FontId::proportional(13.0),
                        theme::TEXTE_SECONDAIRE,
                    );
                }
            }
        });
        if let Some(Couche {
            rendu: Rendu::Image { fenetre },
            ..
        }) = couches.first()
        {
            self.fenetre = *fenetre;
        }
    }

    /// `desc-surfqc.tsv` s'il existe ; sinon la topologie et l'aire calculées par surfoxide à la lecture.
    fn controle_ui(&self, ui: &mut Ui, statuts: &[Statut<Maillage>]) {
        match &self.controle {
            Some(Ok(lignes)) => {
                ui.label(
                    RichText::new("desc-surfqc.tsv (surfx)")
                        .small()
                        .color(theme::TEXTE_SECONDAIRE),
                );
                let petit = |v: String| RichText::new(v).small().monospace();
                let manque = || "—".to_owned();
                egui::Grid::new("surfqc").striped(true).num_columns(7).show(ui, |ui| {
                    for t in ["surface", "sommets", "χ", "auto-int.", "aire (cm²)", "GI", "dist. (mm)"] {
                        ui.label(RichText::new(t).small().strong());
                    }
                    ui.end_row();
                    for l in lignes {
                        ui.label(RichText::new(&l.surface).small());
                        ui.label(petit(l.n_vertices.map_or_else(manque, |n| n.to_string())));
                        let euler = petit(l.euler.map_or_else(manque, |e| e.to_string()));
                        ui.label(if l.euler == Some(2) {
                            euler.color(theme::TERMINE)
                        } else {
                            euler.color(theme::ACCENT)
                        });
                        let auto = petit(l.self_intersections.map_or_else(manque, |n| n.to_string()));
                        ui.label(if l.self_intersections == Some(0) {
                            auto.color(theme::TERMINE)
                        } else {
                            auto.color(theme::ERREUR)
                        });
                        ui.label(petit(
                            l.area_mm2
                                .map_or_else(manque, |a| format!("{:.1}", a / 100.0).replace('.', ",")),
                        ));
                        ui.label(petit(l.gi.map_or_else(manque, |g| format!("{g:.3}").replace('.', ","))));
                        ui.label(petit(
                            l.mean_dist_to_seg_mm
                                .map_or_else(manque, |d| format!("{d:.3}").replace('.', ",")),
                        ));
                        ui.end_row();
                    }
                });
                return;
            }
            Some(Err(e)) => {
                ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
            }
            None => {
                ui.label(
                    RichText::new("desc-surfqc.tsv absent : topologie et aire calculées par surfoxide")
                        .small()
                        .color(theme::TEXTE_SECONDAIRE),
                );
            }
        }
        egui::Grid::new("mesures").striped(true).num_columns(5).show(ui, |ui| {
            for t in ["surface", "sommets", "faces", "χ", "aire (cm²)"] {
                ui.label(RichText::new(t).small().strong());
            }
            ui.end_row();
            for (ligne, statut) in self.lignes.iter().zip(statuts) {
                let Some(m) = statut.pret() else { continue };
                let (Some(topo), Some(aire)) = (&m.topologie, m.aire_mm2) else {
                    continue;
                };
                let hemi = ligne.hemi.unwrap_or("LR");
                ui.label(RichText::new(format!("{} {hemi}", ligne.libelle)).small());
                ui.label(RichText::new(topo.n_vertices.to_string()).small().monospace());
                ui.label(RichText::new(topo.n_faces.to_string()).small().monospace());
                let euler = RichText::new(topo.euler_characteristic.to_string()).small().monospace();
                // Surface fermée, orientée, d'un seul tenant et de genre 0 (sphère) : en bleu.
                let sphere = topo.genus() == Some(0);
                ui.label(if sphere {
                    euler.color(theme::TERMINE)
                } else {
                    euler.color(theme::ACCENT)
                })
                .on_hover_text(format!(
                    "composantes {}, bords {}, arêtes non variétés {}, genre {}",
                    topo.components,
                    topo.boundary_edges,
                    topo.non_manifold_edges,
                    topo.genus().map_or("—".into(), |g| g.to_string())
                ));
                ui.label(
                    RichText::new(format!("{:.1}", aire / 100.0).replace('.', ","))
                        .small()
                        .monospace(),
                );
                ui.end_row();
            }
        });
    }
}

/// Barre de couleur horizontale avec ses bornes.
fn barre_de_couleur(ui: &mut Ui, bas: f32, haut: f32, divergent: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(240.0), 14.0), egui::Sense::hover());
    let n = 64;
    for k in 0..n {
        let t = (k as f32 + 0.5) / n as f32;
        let [r, g, b] = if divergent { divergente(t) } else { sequentielle(t) };
        let x0 = rect.left() + rect.width() * k as f32 / n as f32;
        let x1 = rect.left() + rect.width() * (k + 1) as f32 / n as f32;
        let bande = egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1 + 0.5, rect.bottom()));
        ui.painter().rect_filled(bande, 0.0, Color32::from_rgb(r, g, b));
    }
    ui.horizontal(|ui| {
        let f = |v: f32| format!("{v:.2}").replace('.', ",");
        ui.label(RichText::new(f(bas)).small().monospace());
        ui.add_space((rect.width() - 80.0).max(0.0));
        ui.label(RichText::new(f(haut)).small().monospace());
    });
}
