//! Onglet 1 · Examen : piles acquises, coupe dans le plan natif, vignettes, qualité par coupe.

use std::sync::Arc;

use egui::{Color32, RichText, Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2};
use egui_plot::{Bar, BarChart, Plot, VLine};
use guioxide_bids::{CheminsSession, Etape, Session};
use guioxide_data::QualiteCoupe;
use guioxide_geom::{Orientation, PointMonde, Volume};
use guioxide_view::coupe::{Couche, Rendu, TypeVue, VueCoupe, vignette};
use guioxide_view::theme;

use crate::etapes::{
    Base, CONTOUR, DonneesSession, decimal2, ligne_couche, message_central, nom_court, panneau_curseur,
};

/// Part de coupes conservées sous laquelle une pile est signalée « à revoir ».
const PART_MINIMALE: f64 = 0.8;

/// Côté des vignettes (pixels) : bandeau et mosaïque.
const VIGNETTE: usize = 96;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pile,
    Mosaique,
}

pub struct Examen {
    pile: usize,
    mode: Mode,
    vue: Option<VueCoupe>,
    fenetre: Option<(f32, f32)>,
    masque_visible: bool,
    /// Seuil du score par coupe : en dessous, la coupe est signalée.
    seuil: f64,
    /// Vignettes de la pile affichée, avec le volume dont elles viennent.
    vignettes: Option<(usize, Vec<TextureHandle>)>,
}

impl Default for Examen {
    fn default() -> Self {
        Examen {
            pile: 0,
            mode: Mode::Pile,
            vue: None,
            fenetre: None,
            masque_visible: true,
            seuil: 0.5,
            vignettes: None,
        }
    }
}

/// Bilan d'une pile d'après le fichier de qualité.
struct Bilan {
    coupes: usize,
    exclues: usize,
}

impl Bilan {
    fn de(lignes: &[&QualiteCoupe], coupes: Option<usize>) -> Option<Bilan> {
        if lignes.is_empty() {
            return None;
        }
        Some(Bilan {
            coupes: coupes.unwrap_or(lignes.len()),
            exclues: lignes.iter().filter(|l| l.excluded).count(),
        })
    }

    /// Part de coupes conservées.
    fn part(&self) -> f64 {
        1.0 - self.exclues as f64 / self.coupes.max(1) as f64
    }
}

impl Examen {
    fn signalee(&self, q: &QualiteCoupe) -> bool {
        q.excluded || (q.registered && q.score_after.is_some_and(|s| s < self.seuil))
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        base: &mut Base,
        donnees: &DonneesSession,
        session: &Session,
        chemins: &CheminsSession,
        curseur: &mut PointMonde,
    ) {
        if session.piles.is_empty() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Aucune pile dans anat/").color(theme::TEXTE_SECONDAIRE));
                });
            });
            return;
        }
        let n = self.pile.min(session.piles.len() - 1);
        let pile = &session.piles[n];
        let (image, masque) = (base.statut(&pile.chemin), base.statut(&chemins.masque(pile)));
        if let Some(v) = image.pret() {
            base.placer_curseur(v, curseur);
        }
        let prefixe = chemins.prefixe().to_owned();
        let qualite = donnees.qualite_pile(&pile.nom);

        let mut couches = Vec::new();
        if let Some(v) = image.pret() {
            couches.push(Couche::image(v.volume.clone()));
            couches[0].rendu = Rendu::Image { fenetre: self.fenetre };
            if self.vue.is_none() {
                self.vue = Some(VueCoupe::new(TypeVue::Native(*v.volume.affine())));
            }
            if self.vignettes.as_ref().is_none_or(|(i, _)| *i != n) {
                let textures = (0..v.volume.dims()[2])
                    .map(|k| {
                        ui.ctx().load_texture(
                            format!("vignette-{k}"),
                            vignette(&v.volume, k, VIGNETTE),
                            TextureOptions::LINEAR,
                        )
                    })
                    .collect();
                self.vignettes = Some((n, textures));
            }
        }
        if let Some(v) = masque.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Contour { couleur: CONTOUR },
                visible: self.masque_visible,
            });
        }

        egui::Panel::left("examen_piles")
            .resizable(true)
            .default_size(330.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.liste_piles(ui, donnees, session, &prefixe, n));
                ui.add_space(8.0);
                ligne_couche(ui, "Pile", "sourcedata", &image);
                ligne_couche(ui, "Masque", "medx-fetalbet", &masque);
            });
        egui::Panel::right("examen_qualite")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.synthese(ui, donnees, session, &qualite);
                    ui.add_space(8.0);
                    if masque.pret().is_some() {
                        ui.checkbox(&mut self.masque_visible, "Contour du masque fetalbet");
                    }
                    ui.add_space(8.0);
                    panneau_curseur(ui, curseur, &[("pile", image.pret()), ("masque", masque.pret())]);
                    ui.add_space(12.0);
                    base.valider(ui, Etape::Examen, chemins);
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(volume) = image.pret().map(|v| v.volume.clone()) else {
                message_central(ui, &[("Pile", &image)]);
                return;
            };
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Pile, "Pile");
                ui.selectable_value(&mut self.mode, Mode::Mosaique, "Mosaïque");
                ui.separator();
                self.curseur_de_coupe(ui, &volume, curseur);
            });
            match self.mode {
                Mode::Pile => {
                    egui::Panel::bottom("examen_bandeau").show(ui, |ui| self.bandeau(ui, &volume, &qualite, curseur));
                    if let Some(vue) = &mut self.vue {
                        let rect = ui.available_rect_before_wrap();
                        vue.ui(ui, rect, &mut couches, &[], curseur);
                    }
                }
                Mode::Mosaique => self.mosaique(ui, &volume, &qualite, curseur),
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

    /// Une ligne par pile : orientation, coupes, résolution, part de coupes conservées, badge.
    fn liste_piles(&mut self, ui: &mut Ui, donnees: &DonneesSession, session: &Session, prefixe: &str, choisie: usize) {
        ui.strong(format!("Piles ({})", session.piles.len()));
        if let Some(e) = donnees.qualite.erreur() {
            ui.label(RichText::new(format!("⚠ {e}")).small().color(theme::ERREUR));
        }
        egui::Grid::new("piles")
            .num_columns(4)
            .striped(true)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                let entete = |ui: &mut Ui, t: &str| ui.label(RichText::new(t).small().color(theme::TEXTE_SECONDAIRE));
                entete(ui, "pile");
                entete(ui, "plan scanner").on_hover_text(
                    "Plan des coupes dans le repère du scanner, déduit de l'affine : en IRM fœtale, ce n'est pas \
                     le plan anatomique du fœtus que désigne le nom de la séquence (ax, cor, sag).",
                );
                entete(ui, "coupes");
                entete(ui, "conservées");
                ui.end_row();
                for (i, p) in session.piles.iter().enumerate() {
                    let geometrie = donnees.geometries.get(&p.nom);
                    let reponse =
                        ui.selectable_label(i == choisie, RichText::new(nom_court(&p.nom, prefixe)).monospace());
                    if reponse.clicked() && i != choisie {
                        self.pile = i;
                        (self.vue, self.fenetre, self.vignettes) = (None, None, None);
                    }
                    reponse.on_hover_text(&p.nom);
                    match geometrie {
                        Some(Ok((dims, affine))) => {
                            let t = affine.tailles();
                            ui.label(Orientation::des_coupes(affine).nom())
                                .on_hover_text("plan dans le repère du scanner, pas le plan anatomique du fœtus");
                            ui.label(RichText::new(format!("{} coupes", dims[2])).color(theme::TEXTE_SECONDAIRE))
                                .on_hover_text(format!(
                                    "{} × {} × {} mm",
                                    decimal2(t[0]),
                                    decimal2(t[1]),
                                    decimal2(t[2])
                                ));
                        }
                        Some(Err(e)) => {
                            ui.label(RichText::new("⚠").color(theme::ERREUR)).on_hover_text(e);
                            ui.label("");
                        }
                        None => {
                            ui.label("");
                            ui.label("");
                        }
                    }
                    let coupes = geometrie.and_then(|g| g.as_ref().ok()).map(|(d, _)| d[2]);
                    match Bilan::de(&donnees.qualite_pile(&p.nom), coupes) {
                        Some(b) => {
                            let texte = format!("{:.0} %", b.part() * 100.0);
                            if b.part() < PART_MINIMALE {
                                ui.label(RichText::new(format!("{texte} · à revoir")).color(theme::ACCENT))
                                    .on_hover_text(format!("{} coupes exclues sur {}", b.exclues, b.coupes));
                            } else {
                                ui.label(RichText::new(texte).color(theme::TERMINE))
                                    .on_hover_text(format!("{} coupes exclues sur {}", b.exclues, b.coupes));
                            }
                        }
                        None => {
                            ui.label(RichText::new("—").color(theme::TEXTE_SECONDAIRE))
                                .on_hover_text("pas de qualité par coupe");
                        }
                    }
                    ui.end_row();
                }
            });
    }

    /// Synthèse de la session et histogramme du score par coupe de la pile choisie.
    fn synthese(&mut self, ui: &mut Ui, donnees: &DonneesSession, session: &Session, qualite: &[&QualiteCoupe]) {
        ui.strong("Synthèse");
        if donnees.qualite.lu().is_none() {
            ui.label(
                RichText::new("Pas de qualité par coupe (desc-svr_slicequality.tsv) dans la sortie choisie")
                    .color(theme::TEXTE_SECONDAIRE),
            );
            return;
        }
        let bilans: Vec<Bilan> = session
            .piles
            .iter()
            .filter_map(|p| {
                let coupes = donnees
                    .geometries
                    .get(&p.nom)
                    .and_then(|g| g.as_ref().ok())
                    .map(|(d, _)| d[2]);
                Bilan::de(&donnees.qualite_pile(&p.nom), coupes)
            })
            .collect();
        let utilisables = bilans.iter().filter(|b| b.part() >= PART_MINIMALE).count();
        let (exclues, total) = bilans.iter().fold((0, 0), |(e, t), b| (e + b.exclues, t + b.coupes));
        ui.label(format!("Piles utilisables : {utilisables} / {}", bilans.len()));
        ui.label(format!("Coupes exclues : {exclues} / {total}"));
        ui.add_space(6.0);
        ui.strong("Score par coupe (pile affichée)");
        let scores: Vec<f64> = qualite
            .iter()
            .filter(|q| q.registered)
            .filter_map(|q| q.score_after)
            .collect();
        if scores.is_empty() {
            ui.label(RichText::new("aucune coupe recalée").color(theme::TEXTE_SECONDAIRE));
            return;
        }
        let largeur = 0.05;
        let mut classes = std::collections::BTreeMap::<i64, usize>::new();
        for s in &scores {
            *classes.entry((s / largeur).floor() as i64).or_default() += 1;
        }
        let seuil = self.seuil;
        let barres: Vec<Bar> = classes
            .iter()
            .map(|(&c, &n)| {
                let x = (c as f64 + 0.5) * largeur;
                let couleur = if x < seuil { theme::ACCENT } else { theme::TERMINE };
                Bar::new(x, n as f64).width(largeur * 0.9).fill(couleur)
            })
            .collect();
        Plot::new("histogramme_scores")
            .height(140.0)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show_background(false)
            .show(ui, |p| {
                p.bar_chart(BarChart::new("coupes", barres));
                p.vline(VLine::new("seuil", seuil).color(theme::ACCENT));
            });
        ui.add(egui::Slider::new(&mut self.seuil, -1.0..=1.0).text("seuil"));
        let signalees = qualite.iter().filter(|q| self.signalee(q)).count();
        ui.label(format!("{signalees} coupe(s) signalée(s) sur {}", qualite.len()));
    }

    /// Numéro de coupe de la pile au curseur, et réglette pour en changer.
    fn curseur_de_coupe(&mut self, ui: &mut Ui, volume: &Arc<Volume>, curseur: &mut PointMonde) {
        let nz = volume.dims()[2];
        let ijk = volume.vers_voxel(curseur.0);
        let mut k = (ijk[2].round().max(0.0) as usize).min(nz - 1);
        if ui.add(egui::Slider::new(&mut k, 0..=nz - 1).text("coupe")).changed() {
            aller_a_la_coupe(volume, k, curseur);
        }
    }

    /// Bandeau de vignettes : coupe courante encadrée, coupes signalées en couleur.
    fn bandeau(&self, ui: &mut Ui, volume: &Arc<Volume>, qualite: &[&QualiteCoupe], curseur: &mut PointMonde) {
        let Some((_, textures)) = &self.vignettes else { return };
        let courante = volume.vers_voxel(curseur.0)[2].round() as i64;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (k, t) in textures.iter().enumerate() {
                    let hauteur = 64.0;
                    let taille = Vec2::new(hauteur * t.aspect_ratio(), hauteur);
                    let cadre = self.cadre(k, k as i64 == courante, qualite);
                    if vignette_cliquable(ui, t, taille, cadre, k, qualite).clicked() {
                        aller_a_la_coupe(volume, k, curseur);
                    }
                }
            });
        });
    }

    fn mosaique(&mut self, ui: &mut Ui, volume: &Arc<Volume>, qualite: &[&QualiteCoupe], curseur: &mut PointMonde) {
        let Some((_, textures)) = &self.vignettes else { return };
        let mut choisie = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (k, t) in textures.iter().enumerate() {
                    let taille = Vec2::new(160.0, 160.0 / t.aspect_ratio().max(0.01));
                    let cadre = self.cadre(k, false, qualite);
                    if vignette_cliquable(ui, t, taille, cadre, k, qualite).clicked() {
                        choisie = Some(k);
                    }
                }
            });
        });
        if let Some(k) = choisie {
            aller_a_la_coupe(volume, k, curseur);
            self.mode = Mode::Pile;
        }
    }

    fn cadre(&self, k: usize, courante: bool, qualite: &[&QualiteCoupe]) -> Option<Color32> {
        let q = qualite.iter().find(|q| q.slice == k);
        if courante {
            Some(theme::TEXTE)
        } else if q.is_some_and(|q| q.excluded) {
            Some(theme::ERREUR)
        } else if q.is_some_and(|q| self.signalee(q)) {
            Some(theme::ACCENT)
        } else {
            None
        }
    }
}

fn vignette_cliquable(
    ui: &mut Ui,
    texture: &TextureHandle,
    taille: Vec2,
    cadre: Option<Color32>,
    k: usize,
    qualite: &[&QualiteCoupe],
) -> egui::Response {
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::click());
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    if let Some(c) = cadre {
        ui.painter()
            .rect_stroke(rect, 0.0, Stroke::new(2.0, c), egui::StrokeKind::Inside);
    }
    ui.painter().text(
        rect.left_top() + Vec2::splat(3.0),
        egui::Align2::LEFT_TOP,
        k.to_string(),
        egui::FontId::monospace(10.0),
        theme::TEXTE_SECONDAIRE,
    );
    let q = qualite.iter().find(|q| q.slice == k);
    let infobulle = match q {
        Some(q) => format!(
            "coupe {k}\nscore {} → {}\n{}{}",
            q.score_before.map_or("—".into(), decimal2),
            q.score_after.map_or("—".into(), decimal2),
            if q.excluded { "exclue" } else { "conservée" },
            q.reason.as_ref().map(|r| format!(" ({r})")).unwrap_or_default()
        ),
        None => format!("coupe {k}"),
    };
    reponse.on_hover_text(infobulle)
}

/// Place le curseur sur la coupe `k` de la pile, en gardant sa position dans le plan des coupes.
fn aller_a_la_coupe(volume: &Volume, k: usize, curseur: &mut PointMonde) {
    let [nx, ny, _] = volume.dims();
    let ijk = volume.vers_voxel(curseur.0);
    let i = ijk[0].clamp(0.0, (nx - 1) as f64);
    let j = ijk[1].clamp(0.0, (ny - 1) as f64);
    curseur.0 = volume.vers_monde([i, j, k as f64]);
}
