//! Onglet 1 · Examen : piles acquises, coupe dans le plan natif, vignettes, qualité par coupe.

use std::sync::Arc;

use egui::{Color32, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2};
use egui_plot::{Bar, BarChart, Plot, VLine};
use guioxide_bids::{CheminsSession, Etape, Session};
use guioxide_data::QualiteCoupe;
use guioxide_geom::{Orientation, PointMonde, Volume};
use guioxide_view::coupe::{
    Couche, Rendu, Superposition, TypeVue, VueCoupe, boite_du_masque, vignette, vignette_cadree,
};
use guioxide_view::theme;

use crate::etapes::{
    Base, CONTOUR, DonneesSession, SourceQualite, Statut, decimal2, ligne_couche, message_central, nom_court,
    panneau_curseur,
};

/// Part de coupes conservées sous laquelle une pile est signalée « à revoir ».
const PART_MINIMALE: f64 = 0.8;

/// Côté des vignettes (pixels de texture) : bandeau et mosaïque, assez fin pour une mosaïque en plein écran.
const VIGNETTE: usize = 320;

/// Plan anatomique visé par l'opérateur, d'après le nom de la séquence (`acq-hastecor` → coronal) ; `None` si le
/// nom ne le dit pas.
fn plan_de_la_sequence(nom: &str) -> Option<&'static str> {
    let acq = nom.split('_').find_map(|m| m.strip_prefix("acq-"))?.to_lowercase();
    if acq.ends_with("ax") || acq.contains("axial") || acq.contains("tra") {
        Some("axial")
    } else if acq.ends_with("cor") || acq.contains("coronal") {
        Some("coronal")
    } else if acq.ends_with("sag") || acq.contains("sagittal") {
        Some("sagittal")
    } else {
        None
    }
}

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
    /// Contour du masque fetalbet sur la pile (touche `O`).
    superposition: Superposition,
    /// Seuil du score par coupe : en dessous, la coupe est signalée.
    seuil: f64,
    /// Qualité affichée, si l'utilisateur en a choisi une ; sinon [`DonneesSession::source_par_defaut`].
    source: Option<SourceQualite>,
    /// Vignettes de la pile affichée : pile, cadrées sur le masque ou non, textures.
    vignettes: Option<(usize, bool, Vec<TextureHandle>)>,
}

impl Default for Examen {
    fn default() -> Self {
        Examen {
            pile: 0,
            // `GUIOXIDE_MOSAIQUE=1` : mosaïque d'emblée (captures de contrôle).
            mode: if std::env::var_os("GUIOXIDE_MOSAIQUE").is_some() {
                Mode::Mosaique
            } else {
                Mode::Pile
            },
            vue: None,
            fenetre: None,
            superposition: Superposition::default(),
            seuil: 0.5,
            source: None,
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

/// Score d'une coupe dans `source` : la concordance aux poses d'en-tête avant reconstruction, le score après recalage (coupes recalées seulement) après.
fn score(q: &QualiteCoupe, source: SourceQualite) -> Option<f64> {
    match source {
        SourceQualite::Avant => q.score_before,
        SourceQualite::Apres => q.registered.then_some(q.score_after).flatten(),
    }
}

/// Nom lisible d'un motif de signalement ou d'exclusion.
fn nom_du_motif(motif: &str) -> String {
    match motif {
        "hors_masque" => "hors masque".into(),
        "masque_insuffisant" => "masque insuffisant".into(),
        "sans_partenaire" => "sans partenaire".into(),
        "signal_faible" => "signal faible".into(),
        "faible_concordance" => "faible concordance".into(),
        autre => autre.replace('_', " "),
    }
}

/// Couleur d'un motif : une par motif connu (le SVR ne recale pas les trois premiers, `signal_faible` est à vérifier).
fn couleur_du_motif(motif: Option<&str>) -> Color32 {
    match motif {
        Some("hors_masque") => Color32::from_rgb(0x9A, 0xA1, 0xAB),
        Some("masque_insuffisant") => Color32::from_rgb(0xD9, 0xA4, 0x41),
        Some("sans_partenaire") => Color32::from_rgb(0xB0, 0x7C, 0xC6),
        Some("signal_faible") => theme::ERREUR,
        Some("faible_concordance") => theme::ACCENT,
        _ => theme::ERREUR,
    }
}

impl Examen {
    /// Source de qualité affichée : le choix de l'utilisateur s'il est disponible, sinon la source par défaut.
    fn source_effective(&self, donnees: &DonneesSession) -> Option<SourceQualite> {
        self.source
            .filter(|s| donnees.fichier_qualite(*s).lu().is_some())
            .or_else(|| donnees.source_par_defaut())
    }

    fn signalee(&self, q: &QualiteCoupe, source: SourceQualite) -> bool {
        q.excluded || score(q, source).is_some_and(|s| s < self.seuil)
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
        let source = self.source_effective(donnees);
        let qualite = source.map_or_else(Vec::new, |s| donnees.qualite_pile(s, &pile.nom));

        self.superposition.raccourcis(ui.ctx());
        let mut couches = Vec::new();
        if let Some(v) = image.pret() {
            couches.push(Couche::image(v.volume.clone()));
            couches[0].rendu = Rendu::Image { fenetre: self.fenetre };
            if self.vue.is_none() {
                self.vue = Some(VueCoupe::new(TypeVue::Native(*v.volume.affine())));
            }
            // Vignettes cadrées sur le cerveau quand le masque est lu (il est sur la grille de la pile).
            let masque_lu = masque.pret().is_some();
            let attendre_masque = matches!(masque, Statut::EnCours);
            if !attendre_masque
                && self
                    .vignettes
                    .as_ref()
                    .is_none_or(|(i, lu, _)| *i != n || *lu != masque_lu)
            {
                let boite = masque
                    .pret()
                    .filter(|m| m.volume.dims() == v.volume.dims())
                    .and_then(|m| boite_du_masque(&m.volume, 0.25));
                let textures = (0..v.volume.dims()[2])
                    .map(|k| {
                        let image = match boite {
                            Some(b) => vignette_cadree(&v.volume, k, VIGNETTE, b),
                            None => vignette(&v.volume, k, VIGNETTE),
                        };
                        ui.ctx()
                            .load_texture(format!("vignette-{k}"), image, TextureOptions::LINEAR)
                    })
                    .collect();
                self.vignettes = Some((n, masque_lu, textures));
            }
        }
        if let Some(v) = masque.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Contour { couleur: CONTOUR },
                visible: self.superposition.visible,
            });
        }

        egui::Panel::left("examen_piles")
            .max_size(480.0)
            .resizable(true)
            .default_size(330.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.liste_piles(ui, donnees, session, &prefixe, n, source));
                ui.add_space(8.0);
                ligne_couche(ui, "Pile", "sourcedata", &image);
                ligne_couche(ui, "Masque", "medx-fetalbet", &masque);
            });
        egui::Panel::right("examen_qualite")
            .max_size(480.0)
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.synthese(ui, donnees, session, &qualite, source);
                    ui.add_space(8.0);
                    if masque.pret().is_some() {
                        ui.checkbox(&mut self.superposition.visible, "Contour du masque fetalbet")
                            .on_hover_text("touche O");
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
                    egui::Panel::bottom("examen_bandeau")
                        .show(ui, |ui| self.bandeau(ui, &volume, &qualite, source, curseur));
                    if let Some(vue) = &mut self.vue {
                        let rect = ui.available_rect_before_wrap();
                        vue.ui(ui, rect, &mut couches, &[], curseur);
                    }
                }
                Mode::Mosaique => self.mosaique(ui, &volume, &qualite, source, curseur),
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
    fn liste_piles(
        &mut self,
        ui: &mut Ui,
        donnees: &DonneesSession,
        session: &Session,
        prefixe: &str,
        choisie: usize,
        source: Option<SourceQualite>,
    ) {
        ui.strong(format!("Piles ({})", session.piles.len()));
        for source in [SourceQualite::Apres, SourceQualite::Avant] {
            if let Some(e) = donnees.fichier_qualite(source).erreur() {
                ui.label(RichText::new(format!("⚠ {e}")).small().color(theme::ERREUR));
            }
        }
        egui::Grid::new("piles")
            .num_columns(4)
            .striped(true)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                let entete = |ui: &mut Ui, t: &str| ui.label(RichText::new(t).small().color(theme::TEXTE_SECONDAIRE));
                entete(ui, "pile");
                entete(ui, "plan").on_hover_text(
                    "Plan anatomique visé, lu dans le nom de la séquence (acq-…ax, cor, sag). Le plan des coupes \
                     dans le repère de l'appareil (affine) est en infobulle : chez le fœtus, il ne dit rien du plan \
                     anatomique.",
                );
                entete(ui, "coupes");
                entete(
                    ui,
                    if source == Some(SourceQualite::Avant) {
                        "non signalées"
                    } else {
                        "conservées"
                    },
                );
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
                            let appareil = Orientation::des_coupes(affine).nom();
                            let aide = format!("repère de l'appareil : {appareil} (pas le plan anatomique du fœtus)");
                            match plan_de_la_sequence(&p.nom) {
                                Some(plan) => ui.label(plan).on_hover_text(aide),
                                None => ui
                                    .label(RichText::new("—").color(theme::TEXTE_SECONDAIRE))
                                    .on_hover_text(format!("le nom de la séquence ne dit pas le plan ; {aide}")),
                            };
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
                    let lignes = source.map_or_else(Vec::new, |s| donnees.qualite_pile(s, &p.nom));
                    match Bilan::de(&lignes, coupes) {
                        Some(b) => {
                            let texte = format!("{:.0} %", b.part() * 100.0);
                            if b.part() < PART_MINIMALE {
                                ui.label(RichText::new(format!("{texte} · à revoir")).color(theme::ACCENT))
                                    .on_hover_text(format!(
                                        "{} coupes signalées ou exclues sur {}",
                                        b.exclues, b.coupes
                                    ));
                            } else {
                                ui.label(RichText::new(texte).color(theme::TERMINE))
                                    .on_hover_text(format!(
                                        "{} coupes signalées ou exclues sur {}",
                                        b.exclues, b.coupes
                                    ));
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

    /// Synthèse de la session, bascule Avant / Après, décompte par motif et histogramme du score par coupe de la pile choisie.
    fn synthese(
        &mut self,
        ui: &mut Ui,
        donnees: &DonneesSession,
        session: &Session,
        qualite: &[&QualiteCoupe],
        source: Option<SourceQualite>,
    ) {
        ui.strong("Synthèse");
        let Some(source) = source else {
            ui.label(
                RichText::new(
                    "Pas de qualité par coupe (desc-svr_slicequality.tsv, ni desc-acq_slicequality.tsv de `medx svr qc`) dans la sortie choisie",
                )
                .color(theme::TEXTE_SECONDAIRE),
            );
            return;
        };
        // bascule, seulement si les deux fichiers existent
        if donnees.qualite.lu().is_some() && donnees.qualite_acquisition.lu().is_some() {
            let mut choix = source;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut choix, SourceQualite::Avant, "Avant reconstruction")
                    .on_hover_text("contrôle des acquisitions (medx svr qc) : desc-acq_slicequality.tsv");
                ui.selectable_value(&mut choix, SourceQualite::Apres, "Après reconstruction")
                    .on_hover_text("qualité après recalage par le SVR : desc-svr_slicequality.tsv");
            });
            if choix != source {
                self.source = Some(choix);
            }
        } else {
            ui.label(
                RichText::new(match source {
                    SourceQualite::Avant => "Contrôle des acquisitions, avant reconstruction",
                    SourceQualite::Apres => "Qualité après reconstruction",
                })
                .small()
                .color(theme::TEXTE_SECONDAIRE),
            );
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
                Bilan::de(&donnees.qualite_pile(source, &p.nom), coupes)
            })
            .collect();
        let utilisables = bilans.iter().filter(|b| b.part() >= PART_MINIMALE).count();
        let (exclues, total) = bilans.iter().fold((0, 0), |(e, t), b| (e + b.exclues, t + b.coupes));
        ui.label(format!("Piles utilisables : {utilisables} / {}", bilans.len()));
        ui.label(format!(
            "{} : {exclues} / {total}",
            if source == SourceQualite::Avant {
                "Coupes signalées"
            } else {
                "Coupes exclues"
            }
        ));
        // décompte par motif, sur toute la session
        let mut motifs = std::collections::BTreeMap::<String, usize>::new();
        if let Some(lignes) = donnees.fichier_qualite(source).lu() {
            for l in lignes.iter().filter(|l| l.excluded) {
                *motifs
                    .entry(l.reason.clone().unwrap_or_else(|| "sans motif".into()))
                    .or_default() += 1;
            }
        }
        let mut motifs: Vec<(String, usize)> = motifs.into_iter().collect();
        motifs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (motif, n) in &motifs {
            ui.horizontal(|ui| {
                ui.label(RichText::new("■").color(couleur_du_motif(Some(motif))));
                ui.label(format!("{} : {n}", nom_du_motif(motif)));
            });
        }
        ui.add_space(6.0);
        ui.strong(match source {
            SourceQualite::Avant => "Concordance d'en-tête par coupe (pile affichée)",
            SourceQualite::Apres => "Score par coupe (pile affichée)",
        });
        let scores: Vec<f64> = qualite.iter().filter_map(|q| score(q, source)).collect();
        if scores.is_empty() {
            ui.label(
                RichText::new(match source {
                    SourceQualite::Avant => "aucune coupe évaluable (sans partenaire ou masque insuffisant)",
                    SourceQualite::Apres => "aucune coupe recalée",
                })
                .color(theme::TEXTE_SECONDAIRE),
            );
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
        let signalees = qualite.iter().filter(|q| self.signalee(q, source)).count();
        ui.label(format!("{signalees} coupe(s) signalée(s) sur {}", qualite.len()));
    }

    /// Numéro de coupe de la pile au curseur, et réglette pour en changer.
    fn curseur_de_coupe(&mut self, ui: &mut Ui, volume: &Arc<Volume>, curseur: &mut PointMonde) {
        let nz = volume.dims()[2];
        let ijk = volume.vers_voxel(curseur.0);
        // Numérotée à partir de 1, comme dans l'annotation de la vue.
        let mut k = (ijk[2].round().max(0.0) as usize).min(nz - 1) + 1;
        if ui
            .add(egui::Slider::new(&mut k, 1..=nz).text(format!("/ {nz} coupes")))
            .changed()
        {
            aller_a_la_coupe(volume, k - 1, curseur);
        }
    }

    /// Bandeau de vignettes : coupe courante encadrée, coupes signalées en couleur.
    fn bandeau(
        &self,
        ui: &mut Ui,
        volume: &Arc<Volume>,
        qualite: &[&QualiteCoupe],
        source: Option<SourceQualite>,
        curseur: &mut PointMonde,
    ) {
        let Some((_, _, textures)) = &self.vignettes else {
            return;
        };
        let courante = volume.vers_voxel(curseur.0)[2].round() as i64;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (k, t) in textures.iter().enumerate() {
                    let hauteur = 64.0;
                    let taille = Vec2::new(hauteur * t.aspect_ratio(), hauteur);
                    let cadre = self.cadre(k, k as i64 == courante, qualite, source);
                    if vignette_cliquable(ui, t, taille, cadre, k, qualite).clicked() {
                        aller_a_la_coupe(volume, k, curseur);
                    }
                }
            });
        });
    }

    fn mosaique(
        &mut self,
        ui: &mut Ui,
        volume: &Arc<Volume>,
        qualite: &[&QualiteCoupe],
        source: Option<SourceQualite>,
        curseur: &mut PointMonde,
    ) {
        let Some((_, _, textures)) = &self.vignettes else {
            return;
        };
        if source.is_some() {
            legende_mosaique(ui, source);
        }
        let mut choisie = None;
        let n = textures.len().max(1);
        let rapport = textures.first().map_or(1.0, |t| t.aspect_ratio().max(0.01));
        let zone = ui.available_size();
        let ecart = 6.0;
        // Nombre de colonnes qui donne les plus grandes vignettes tenant dans la zone ; au-dessous de 140 points de
        // large, la mosaïque défile.
        let largeur = (1..=n)
            .map(|c| {
                let lignes = n.div_ceil(c) as f32;
                let l = (zone.x - ecart * (c as f32 - 1.0)) / c as f32;
                let h = (zone.y - ecart * (lignes - 1.0)) / lignes;
                l.min(h * rapport)
            })
            .fold(0.0_f32, f32::max)
            .max(140.0)
            .min(zone.x);
        let colonnes = (((zone.x + ecart) / (largeur + ecart)).floor() as usize).max(1);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(ecart);
            for rangee in textures.chunks(colonnes).enumerate() {
                let (r, ligne) = rangee;
                ui.horizontal(|ui| {
                    for (j, t) in ligne.iter().enumerate() {
                        let k = r * colonnes + j;
                        let taille = Vec2::new(largeur, largeur / t.aspect_ratio().max(0.01));
                        let cadre = self.cadre(k, false, qualite, source);
                        if vignette_cliquable(ui, t, taille, cadre, k, qualite).clicked() {
                            choisie = Some(k);
                        }
                    }
                });
            }
        });
        if let Some(k) = choisie {
            aller_a_la_coupe(volume, k, curseur);
            self.mode = Mode::Pile;
        }
    }

    /// Cadre d'une vignette : la coupe courante en clair ; une coupe signalée ou exclue dans la couleur de son motif ; une coupe sous le seuil en orange.
    fn cadre(
        &self,
        k: usize,
        courante: bool,
        qualite: &[&QualiteCoupe],
        source: Option<SourceQualite>,
    ) -> Option<Color32> {
        let q = qualite.iter().find(|q| q.slice == k);
        if courante {
            Some(theme::TEXTE)
        } else if let Some(q) = q.filter(|q| q.excluded) {
            Some(couleur_du_motif(q.reason.as_deref()))
        } else if source.is_some_and(|s| q.is_some_and(|q| self.signalee(q, s))) {
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
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    if let Some(c) = cadre {
        ui.painter()
            .rect_stroke(rect, 0.0, Stroke::new(2.0, c), egui::StrokeKind::Inside);
    }
    let grande = taille.x >= 120.0;
    ui.painter().text(
        rect.left_top() + Vec2::splat(4.0),
        egui::Align2::LEFT_TOP,
        k.to_string(),
        egui::FontId::monospace(if grande { 12.0 } else { 10.0 }),
        theme::TEXTE_SECONDAIRE,
    );
    let q = qualite.iter().find(|q| q.slice == k);
    // Marqueur : utilisée (✓) ou exclue (✗, avec le motif sur les grandes vignettes).
    if let Some(q) = q {
        let (texte, couleur) = if q.excluded {
            let motif = q.reason.as_deref().map(nom_du_motif).unwrap_or_default();
            let texte = if grande {
                format!("✗ {motif}")
            } else {
                "✗".to_owned()
            };
            (texte, couleur_du_motif(q.reason.as_deref()))
        } else if q.registered {
            ("✓".to_owned(), theme::TERMINE)
        } else {
            ("·".to_owned(), theme::TEXTE_SECONDAIRE)
        };
        let police = egui::FontId::proportional(if grande { 13.0 } else { 11.0 });
        let galee = ui.painter().layout_no_wrap(texte, police, couleur);
        let coin = rect.left_bottom() + Vec2::new(4.0, -4.0 - galee.size().y);
        let fond = Rect::from_min_size(coin - Vec2::splat(3.0), galee.size() + Vec2::splat(6.0));
        ui.painter().rect_filled(fond, 4.0, Color32::from_black_alpha(170));
        ui.painter().galley(coin, galee, couleur);
    }
    let infobulle = match q {
        Some(q) => format!(
            "coupe {k}\nscore {} → {}\n{}{}",
            q.score_before.map_or("—".into(), decimal2),
            q.score_after.map_or("—".into(), decimal2),
            if q.excluded {
                "exclue de la reconstruction"
            } else if q.registered {
                "utilisée (recalée)"
            } else {
                "non recalée"
            },
            q.reason
                .as_deref()
                .map(|r| format!(" ({})", nom_du_motif(r)))
                .unwrap_or_default()
        ),
        None => format!("coupe {k}"),
    };
    reponse.on_hover_text(infobulle)
}

/// Légende de la mosaïque : marqueurs et motifs d'exclusion.
fn legende_mosaique(ui: &mut Ui, source: Option<SourceQualite>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("✓").color(theme::TERMINE));
        ui.label(RichText::new("utilisée").small().color(theme::TEXTE_SECONDAIRE));
        ui.label(RichText::new("✗").color(theme::ERREUR));
        ui.label(RichText::new("exclue").small().color(theme::TEXTE_SECONDAIRE));
        ui.label(
            RichText::new(match source {
                Some(SourceQualite::Avant) => "· contrôle des acquisitions (avant reconstruction)",
                _ => "· d'après la reconstruction",
            })
            .small()
            .color(theme::TEXTE_SECONDAIRE),
        );
        ui.label(
            RichText::new("■ hors masque")
                .small()
                .color(couleur_du_motif(Some("hors_masque"))),
        )
        .on_hover_text(
            "Le masque du cerveau (medx fetalbet) est vide sur cette coupe : le SVR ne s'en sert pas. C'est \
                 normal pour les premières et dernières coupes d'une pile, qui passent à côté du cerveau ; si une \
                 coupe qui montre le cerveau est « hors masque », c'est le masque qui est à revoir.",
        );
    });
    ui.add_space(4.0);
}

/// Place le curseur sur la coupe `k` de la pile, en gardant sa position dans le plan des coupes.
fn aller_a_la_coupe(volume: &Volume, k: usize, curseur: &mut PointMonde) {
    let [nx, ny, _] = volume.dims();
    let ijk = volume.vers_voxel(curseur.0);
    let i = ijk[0].clamp(0.0, (nx - 1) as f64);
    let j = ijk[1].clamp(0.0, (ny - 1) as f64);
    curseur.0 = volume.vers_monde([i, j, k as f64]);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::etapes::Fichier;

    fn ligne(
        registered: bool,
        avant: Option<f64>,
        apres: Option<f64>,
        excluded: bool,
        motif: Option<&str>,
    ) -> QualiteCoupe {
        QualiteCoupe {
            stack: "p".into(),
            slice: 0,
            registered,
            score_before: avant,
            score_after: apres,
            n_registrations: None,
            correction_rms_mm: None,
            excluded,
            reason: motif.map(str::to_owned),
        }
    }

    fn donnees(apres: bool, avant: bool) -> DonneesSession {
        let fichier = |present: bool| {
            if present {
                Fichier::Lu(vec![ligne(false, None, None, false, None)])
            } else {
                Fichier::Absent
            }
        };
        DonneesSession {
            qualite: fichier(apres),
            qualite_acquisition: fichier(avant),
            mouvement: Fichier::Absent,
            parametres: Fichier::Absent,
            geometries: HashMap::new(),
        }
    }

    /// Le score tracé : la concordance d'en-tête avant reconstruction (même pour une coupe non recalée), le score après recalage, des coupes recalées seulement, après.
    #[test]
    fn the_score_depends_on_the_source() {
        let non_recalee = ligne(false, Some(0.7), None, false, None);
        assert_eq!(score(&non_recalee, SourceQualite::Avant), Some(0.7));
        assert_eq!(score(&non_recalee, SourceQualite::Apres), None);
        let recalee = ligne(true, Some(0.4), Some(0.9), false, None);
        assert_eq!(score(&recalee, SourceQualite::Avant), Some(0.4));
        assert_eq!(score(&recalee, SourceQualite::Apres), Some(0.9));
    }

    /// Source affichée : après reconstruction si elle existe, sinon le contrôle ; un choix de l'utilisateur n'est suivi que si le fichier existe ; aucune source sans fichier.
    #[test]
    fn the_displayed_source_follows_the_files_that_exist() {
        let mut examen = Examen::default();
        assert_eq!(
            examen.source_effective(&donnees(true, true)),
            Some(SourceQualite::Apres)
        );
        assert_eq!(
            examen.source_effective(&donnees(false, true)),
            Some(SourceQualite::Avant)
        );
        assert_eq!(
            examen.source_effective(&donnees(true, false)),
            Some(SourceQualite::Apres)
        );
        assert_eq!(examen.source_effective(&donnees(false, false)), None);
        examen.source = Some(SourceQualite::Avant);
        assert_eq!(
            examen.source_effective(&donnees(true, true)),
            Some(SourceQualite::Avant)
        );
        assert_eq!(
            examen.source_effective(&donnees(true, false)),
            Some(SourceQualite::Apres),
            "le choix d'un fichier absent n'est pas suivi"
        );
    }

    /// Une couleur par motif : les quatre motifs du contrôle et la faible concordance du SVR sont deux à deux différents, et un motif inconnu garde la couleur d'erreur ; une coupe signalée sous le seuil, sans motif, est orange.
    #[test]
    fn each_reason_has_its_own_colour() {
        let motifs = [
            "hors_masque",
            "masque_insuffisant",
            "sans_partenaire",
            "signal_faible",
            "faible_concordance",
        ];
        let couleurs: Vec<Color32> = motifs.iter().map(|m| couleur_du_motif(Some(m))).collect();
        for i in 0..couleurs.len() {
            for j in i + 1..couleurs.len() {
                assert_ne!(couleurs[i], couleurs[j], "{} et {}", motifs[i], motifs[j]);
            }
        }
        assert_eq!(couleur_du_motif(Some("autre")), theme::ERREUR);
        let examen = Examen {
            seuil: 0.5,
            ..Examen::default()
        };
        let q = [&ligne(true, Some(0.8), Some(0.2), false, None)];
        assert_eq!(
            examen.cadre(0, false, &q, Some(SourceQualite::Apres)),
            Some(theme::ACCENT)
        );
        let exclue = [&ligne(false, None, None, true, Some("sans_partenaire"))];
        assert_eq!(
            examen.cadre(0, false, &exclue, Some(SourceQualite::Avant)),
            Some(couleur_du_motif(Some("sans_partenaire")))
        );
        assert_eq!(
            examen.cadre(0, true, &exclue, Some(SourceQualite::Avant)),
            Some(theme::TEXTE)
        );
    }

    #[test]
    fn plan_lu_dans_le_nom_de_la_sequence() {
        assert_eq!(plan_de_la_sequence("sub-a_ses-1_acq-hastecor_T2w"), Some("coronal"));
        assert_eq!(
            plan_de_la_sequence("sub-a_ses-1_acq-trufisag_run-1_T2w"),
            Some("sagittal")
        );
        assert_eq!(plan_de_la_sequence("sub-a_ses-1_acq-hasteax_T2w"), Some("axial"));
        assert_eq!(plan_de_la_sequence("sub-a_ses-1_run-1_T2w"), None);
    }

    #[test]
    fn the_reason_names_are_readable() {
        assert_eq!(nom_du_motif("hors_masque"), "hors masque");
        assert_eq!(nom_du_motif("masque_insuffisant"), "masque insuffisant");
        assert_eq!(nom_du_motif("raison_inconnue"), "raison inconnue");
    }
}
