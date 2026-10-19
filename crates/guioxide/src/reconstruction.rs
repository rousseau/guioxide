//! Onglet 2 · Reconstruction : sous-étapes (masques, SVR et mouvement, segmentation), vue triplanaire, courbe de
//! mouvement par coupe, table des étiquettes avec leurs volumes.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{RichText, Ui};
use egui_plot::{Legend, Line, Plot, PlotPoints, Points, VLine};
use guioxide_bids::{CheminsSession, EtatEtape, Session};
use guioxide_data::{PointMouvement, TableEtiquettes, mouvement_par_coupe};
use guioxide_geom::{PointMonde, Volume};
use guioxide_view::coupe::{Couche, Rendu, Triplanaire};
use guioxide_view::{pastille_etat, theme};

use crate::etapes::{
    Base, DonneesSession, Fichier, Statut, bouton_valider, ligne_couche, message_central, nom_court, panneau_curseur,
};

/// Opacité initiale de la segmentation.
const OPACITE: f32 = 0.45;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SousEtape {
    Masques,
    Svr,
    Segmentation,
}

impl SousEtape {
    const TOUTES: [SousEtape; 3] = [SousEtape::Masques, SousEtape::Svr, SousEtape::Segmentation];

    fn libelle(self) -> &'static str {
        match self {
            SousEtape::Masques => "2a Masques",
            SousEtape::Svr => "2b Reconstruction et mouvement",
            SousEtape::Segmentation => "2c Segmentation",
        }
    }
}

pub struct Reconstruction {
    sous_etape: SousEtape,
    triplanaire: Triplanaire,
    fenetre: Option<(f32, f32)>,
    segmentation_visible: bool,
    opacite: f32,
    etiquettes: Option<Fichier<TableEtiquettes>>,
    /// Volume (cm³) de chaque étiquette, calculé une fois par segmentation lue.
    volumes: Option<(usize, BTreeMap<u32, f64>)>,
    /// Pile montrée sur la courbe de mouvement ; `None` : toutes, bout à bout.
    pile_mouvement: Option<String>,
    mouvement: Option<Vec<PointMouvement>>,
}

impl Default for Reconstruction {
    fn default() -> Self {
        Reconstruction {
            sous_etape: SousEtape::Svr,
            triplanaire: Triplanaire::default(),
            fenetre: None,
            segmentation_visible: true,
            opacite: OPACITE,
            etiquettes: None,
            volumes: None,
            pile_mouvement: None,
            mouvement: None,
        }
    }
}

/// Mouvement par coupe de la session (voir [`guioxide_data::mouvement_par_coupe`]).
fn mouvement_session(donnees: &DonneesSession) -> Vec<PointMouvement> {
    let Some(lignes) = donnees.mouvement.lu() else {
        return Vec::new();
    };
    let geometrie = |pile: &str| donnees.geometries.get(pile).and_then(|g| g.as_ref().ok()).copied();
    mouvement_par_coupe(lignes, geometrie, donnees.qualite.lu().map(Vec::as_slice))
}

/// Volume (cm³) de chaque étiquette non nulle.
fn volumes_etiquettes(volume: &Volume) -> BTreeMap<u32, f64> {
    let mut comptes = BTreeMap::<u32, usize>::new();
    for v in volume.voxels() {
        let i = v.round();
        if i >= 1.0 {
            *comptes.entry(i as u32).or_default() += 1;
        }
    }
    let voxel_cm3 = volume.affine().volume_voxel() / 1000.0;
    comptes.into_iter().map(|(i, n)| (i, n as f64 * voxel_cm3)).collect()
}

impl Reconstruction {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        base: &mut Base,
        donnees: &DonneesSession,
        session: &Session,
        chemins: &CheminsSession,
        curseur: &mut PointMonde,
    ) {
        let (svr, seg) = (base.statut(&chemins.svr()), base.statut(&chemins.segmentation()));
        let total_t2w = session.piles.iter().filter(|p| p.contraste == "T2w").count();
        let masques_presents = session
            .piles
            .iter()
            .filter(|p| p.contraste == "T2w" && chemins.masque(p).exists())
            .count();
        let etiquettes = self.etiquettes.get_or_insert_with(|| {
            let tsv = chemins.etiquettes();
            if tsv.exists() {
                match TableEtiquettes::lire(&tsv) {
                    Ok(t) => Fichier::Lu(t),
                    Err(e) => Fichier::Erreur(e.to_string()),
                }
            } else {
                Fichier::Absent
            }
        });
        let table = etiquettes.lu().cloned().unwrap_or_default();
        let erreur_etiquettes = etiquettes.erreur().map(str::to_owned);
        if let Some(v) = svr.pret().or(seg.pret()) {
            base.placer_curseur(v, curseur);
        }
        if let Some(v) = seg.pret()
            && self
                .volumes
                .as_ref()
                .is_none_or(|(p, _)| *p != Arc::as_ptr(&v.volume) as usize)
        {
            self.volumes = Some((Arc::as_ptr(&v.volume) as usize, volumes_etiquettes(&v.volume)));
        }
        let mouvement = self.mouvement.get_or_insert_with(|| mouvement_session(donnees));

        let mut couches = Vec::new();
        if let Some(v) = svr.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Image { fenetre: self.fenetre },
                visible: true,
            });
        }
        if let Some(v) = seg.pret() {
            couches.push(Couche {
                volume: v.volume.clone(),
                rendu: Rendu::Etiquettes {
                    table: table.clone(),
                    opacite: self.opacite,
                },
                visible: self.segmentation_visible,
            });
        }

        let etat = |s: SousEtape| match s {
            SousEtape::Masques => match masques_presents {
                0 => EtatEtape::NonFaite,
                n if n == total_t2w => EtatEtape::Terminee,
                _ => EtatEtape::EnCours,
            },
            SousEtape::Svr => match (&svr, donnees.mouvement.erreur()) {
                (Statut::Erreur(e), _) => EtatEtape::Erreur(e.clone()),
                (_, Some(e)) => EtatEtape::Erreur(e.to_owned()),
                (Statut::Absent, _) => EtatEtape::NonFaite,
                _ => EtatEtape::Terminee,
            },
            SousEtape::Segmentation => match &seg {
                Statut::Erreur(e) => EtatEtape::Erreur(e.clone()),
                Statut::Absent => EtatEtape::NonFaite,
                _ => EtatEtape::Terminee,
            },
        };

        egui::Panel::top("reco_sous_etapes").show(ui, |ui| {
            ui.horizontal(|ui| {
                for s in SousEtape::TOUTES {
                    pastille_etat(ui, &etat(s));
                    ui.selectable_value(&mut self.sous_etape, s, s.libelle());
                    ui.add_space(12.0);
                }
            });
        });
        egui::Panel::left("reco_couches")
            .resizable(true)
            .default_size(290.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong("Couches");
                    ligne_couche(ui, "Volume reconstruit", "medx-svr", &svr);
                    ligne_couche(ui, "Segmentation", "medx-bounti", &seg);
                    if seg.pret().is_some() {
                        ui.checkbox(&mut self.segmentation_visible, "Afficher la segmentation");
                        ui.add(egui::Slider::new(&mut self.opacite, 0.0..=1.0).text("opacité"));
                    }
                    if svr.pret().is_some() && ui.button("Fenêtrage automatique").clicked() {
                        self.fenetre = None;
                    }
                    if let Some(e) = &erreur_etiquettes {
                        ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
                    }
                    if let Some((_, volumes)) = &self.volumes {
                        ui.add_space(8.0);
                        ui.strong("Étiquettes");
                        table_etiquettes(ui, &table, volumes);
                    }
                });
            });
        egui::Panel::right("reco_parametres")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong(self.sous_etape.libelle());
                    match self.sous_etape {
                        SousEtape::Masques => {
                            ui.label(format!("Masques fetalbet : {masques_presents} / {total_t2w} piles T2w"));
                            for p in session.piles.iter().filter(|p| p.contraste == "T2w") {
                                let present = chemins.masque(p).exists();
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        if present { "✓" } else { "○" },
                                        nom_court(&p.nom, chemins.prefixe())
                                    ))
                                    .color(if present {
                                        theme::TERMINE
                                    } else {
                                        theme::TEXTE_SECONDAIRE
                                    }),
                                );
                            }
                        }
                        SousEtape::Svr => {
                            parametres(ui, &donnees.parametres);
                            ui.add_space(6.0);
                            resume_mouvement(ui, mouvement, donnees);
                        }
                        SousEtape::Segmentation => {
                            if let Some((_, volumes)) = &self.volumes {
                                let total: f64 = volumes.values().sum();
                                ui.label(format!(
                                    "Volume segmenté : {} cm³",
                                    format!("{total:.1}").replace('.', ",")
                                ));
                            } else {
                                ui.label(RichText::new("pas de segmentation lue").color(theme::TEXTE_SECONDAIRE));
                            }
                        }
                    }
                    ui.add_space(8.0);
                    panneau_curseur(ui, curseur, &[("volume", svr.pret()), ("segmentation", seg.pret())]);
                    ui.add_space(12.0);
                    bouton_valider(ui);
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if couches.is_empty() {
                message_central(ui, &[("Volume reconstruit", &svr), ("Segmentation", &seg)]);
                return;
            }
            let pile_mouvement = &mut self.pile_mouvement;
            self.triplanaire.ui(ui, &mut couches, &[], curseur, |ui, case| {
                let mut enfant = ui.new_child(egui::UiBuilder::new().max_rect(case.shrink(4.0)));
                courbe_mouvement(&mut enfant, mouvement, pile_mouvement, chemins.prefixe(), donnees);
            });
        });
        if let Some(Couche {
            rendu: Rendu::Image { fenetre },
            ..
        }) = couches.first()
        {
            self.fenetre = *fenetre;
        }
    }
}

/// Paramètres de la reconstruction (`desc-svr_T2w.json`), clé par clé.
fn parametres(ui: &mut Ui, fichier: &Fichier<serde_json::Value>) {
    match fichier {
        Fichier::Absent => {
            ui.label(RichText::new("Paramètres : desc-svr_T2w.json absent").color(theme::TEXTE_SECONDAIRE));
        }
        Fichier::Erreur(e) => {
            ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
        }
        Fichier::Lu(valeur) => {
            egui::Grid::new("parametres_svr")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    let entrees: Vec<(String, &serde_json::Value)> = match valeur.as_object() {
                        Some(o) => o.iter().map(|(k, v)| (k.clone(), v)).collect(),
                        None => vec![("valeur".to_owned(), valeur)],
                    };
                    for (cle, v) in entrees {
                        ui.label(RichText::new(cle).monospace());
                        let texte = match v {
                            serde_json::Value::String(s) => s.clone(),
                            autre => autre.to_string(),
                        };
                        ui.label(texte);
                        ui.end_row();
                    }
                });
        }
    }
}

fn resume_mouvement(ui: &mut Ui, mouvement: &[PointMouvement], donnees: &DonneesSession) {
    match &donnees.mouvement {
        Fichier::Absent => {
            ui.label(RichText::new("Mouvement : desc-svr_motion.tsv absent").color(theme::TEXTE_SECONDAIRE));
        }
        Fichier::Erreur(e) => {
            ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
        }
        Fichier::Lu(lignes) => {
            let max_t = mouvement.iter().map(|p| p.translation).fold(0.0, f64::max);
            let max_r = mouvement.iter().map(|p| p.rotation).fold(0.0, f64::max);
            let moy_t = mouvement.iter().map(|p| p.translation).sum::<f64>() / mouvement.len().max(1) as f64;
            ui.label(format!(
                "{} coupes · translation moyenne {} mm, maximale {} mm",
                mouvement.len(),
                virgule(moy_t),
                virgule(max_t)
            ));
            ui.label(format!("rotation maximale {}°", virgule(max_r)));
            let omises = lignes.len() - mouvement.len();
            if omises > 0 {
                ui.label(
                    RichText::new(format!(
                        "{omises} coupe(s) omise(s) : pile inconnue ou géométrie illisible"
                    ))
                    .color(theme::ACCENT),
                );
            }
        }
    }
}

fn virgule(x: f64) -> String {
    format!("{x:.1}").replace('.', ",")
}

/// Courbe de mouvement : translation (mm) et rotation (°) par coupe ; coupes exclues en couleur ; une pile ou toutes.
fn courbe_mouvement(
    ui: &mut Ui,
    mouvement: &[PointMouvement],
    pile: &mut Option<String>,
    prefixe: &str,
    donnees: &DonneesSession,
) {
    if mouvement.is_empty() {
        let texte = match &donnees.mouvement {
            Fichier::Lu(_) => "Mouvement : aucune coupe d'une pile connue",
            Fichier::Erreur(_) => "Mouvement illisible (voir les paramètres)",
            Fichier::Absent => "Pas de mouvement (desc-svr_motion.tsv) dans la sortie choisie",
        };
        ui.centered_and_justified(|ui| ui.label(RichText::new(texte).color(theme::TEXTE_SECONDAIRE)));
        return;
    }
    let mut piles: Vec<&str> = mouvement.iter().map(|p| p.stack.as_str()).collect();
    piles.dedup();
    ui.horizontal(|ui| {
        ui.label("Mouvement");
        egui::ComboBox::from_id_salt("pile_mouvement")
            .selected_text(pile.as_deref().map_or("toutes les piles", |p| nom_court(p, prefixe)))
            .show_ui(ui, |ui| {
                ui.selectable_value(pile, None, "toutes les piles");
                for p in &piles {
                    ui.selectable_value(pile, Some((*p).to_owned()), nom_court(p, prefixe));
                }
            });
    });
    // Abscisse : indice de coupe dans la pile, ou rang global quand toutes les piles sont mises bout à bout.
    let choisis: Vec<(f64, &PointMouvement)> = match pile {
        Some(p) => mouvement
            .iter()
            .filter(|m| &m.stack == p)
            .map(|m| (m.slice as f64, m))
            .collect(),
        None => mouvement.iter().enumerate().map(|(i, m)| (i as f64, m)).collect(),
    };
    let serie = |f: fn(&PointMouvement) -> f64| PlotPoints::new(choisis.iter().map(|(x, m)| [*x, f(m)]).collect());
    let exclues: Vec<[f64; 2]> = choisis
        .iter()
        .filter(|(_, m)| m.exclue)
        .map(|(x, m)| [*x, m.translation])
        .collect();
    let limites: Vec<f64> = if pile.is_none() {
        choisis
            .windows(2)
            .filter(|w| w[0].1.stack != w[1].1.stack)
            .map(|w| w[1].0 - 0.5)
            .collect()
    } else {
        Vec::new()
    };
    Plot::new("courbe_mouvement")
        .legend(Legend::default())
        .x_axis_label(if pile.is_some() {
            "coupe"
        } else {
            "coupe (piles bout à bout)"
        })
        .show_background(false)
        .show(ui, |p| {
            p.line(Line::new("translation (mm)", serie(|m| m.translation)).color(theme::TERMINE));
            p.line(Line::new("rotation (°)", serie(|m| m.rotation)).color(theme::ACCENT));
            if !exclues.is_empty() {
                p.points(Points::new("exclues", exclues).color(theme::ERREUR).radius(3.0));
            }
            for x in limites {
                p.vline(VLine::new("", x).color(theme::BORDURE));
            }
        });
}

/// Étiquettes : couleur, indice, nom, volume (cm³).
fn table_etiquettes(ui: &mut Ui, table: &TableEtiquettes, volumes: &BTreeMap<u32, f64>) {
    let mut indices: std::collections::BTreeSet<u32> = table.etiquettes.keys().copied().filter(|&i| i > 0).collect();
    indices.extend(volumes.keys());
    egui::Grid::new("etiquettes")
        .num_columns(4)
        .spacing([6.0, 2.0])
        .show(ui, |ui| {
            for i in indices {
                let [r, g, b] = table.couleur(i);
                let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                ui.label(RichText::new(i.to_string()).monospace());
                let e = table.etiquettes.get(&i);
                let nom = e
                    .map(|e| e.abreviation.clone().unwrap_or_else(|| e.nom.clone()))
                    .unwrap_or_default();
                let reponse = ui.label(nom);
                if let Some(e) = e {
                    reponse.on_hover_text(&e.nom);
                }
                let v = volumes.get(&i).copied().unwrap_or(0.0);
                ui.label(
                    RichText::new(format!("{} cm³", format!("{v:.2}").replace('.', ",")))
                        .monospace()
                        .color(theme::TEXTE_SECONDAIRE),
                );
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use guioxide_geom::Affine;

    use guioxide_data::{MouvementCoupe, QualiteCoupe};
    use std::collections::HashMap;

    #[test]
    fn mouvement_par_coupe_au_centre_de_chaque_coupe() {
        // Pile 3 × 3 × 2 de voxels 1 mm, centre de la coupe k en (1, 1, k).
        let geometries = HashMap::from([("p".to_owned(), Ok(([3, 3, 2], Affine::IDENTITE)))]);
        // Coupe 0 : translation (3, 4, 0) ; coupe 1 : rotation de 90° autour de z (autour de l'origine monde).
        let mut t = Affine::IDENTITE;
        (t.0[0][3], t.0[1][3]) = (3.0, 4.0);
        let r = Affine([
            [0.0, -1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let ligne = |stack: &str, slice, matrice| MouvementCoupe {
            stack: stack.into(),
            slice,
            matrice,
        };
        let qualite = |slice, excluded| QualiteCoupe {
            stack: "p".into(),
            slice,
            registered: true,
            score_before: None,
            score_after: None,
            n_registrations: None,
            correction_rms_mm: None,
            excluded,
            reason: None,
        };
        let donnees = DonneesSession {
            qualite: Fichier::Lu(vec![qualite(0, false), qualite(1, true)]),
            mouvement: Fichier::Lu(vec![ligne("p", 0, t), ligne("p", 1, r), ligne("inconnue", 0, t)]),
            parametres: Fichier::Absent,
            geometries,
        };
        let m = mouvement_session(&donnees);
        assert_eq!(m.len(), 2, "la pile sans géométrie est omise");
        assert!((m[0].translation - 5.0).abs() < 1e-12 && m[0].rotation == 0.0 && !m[0].exclue);
        // (1, 1, 1) → (−1, 1, 1) : déplacement 2 mm, rotation 90°.
        assert!((m[1].translation - 2.0).abs() < 1e-12 && (m[1].rotation - 90.0).abs() < 1e-9 && m[1].exclue);
    }

    #[test]
    fn volumes_des_etiquettes_en_cm3() {
        let mut a = Affine::IDENTITE;
        (a.0[0][0], a.0[1][1], a.0[2][2]) = (2.0, 5.0, 10.0); // 100 mm³ = 0,1 cm³ par voxel
        let vol = Volume::new([4, 1, 1], a, vec![0.0, 1.0, 1.0, 3.0]).unwrap();
        let v = volumes_etiquettes(&vol);
        assert_eq!(v.len(), 2);
        assert!((v[&1] - 0.2).abs() < 1e-12 && (v[&3] - 0.1).abs() < 1e-12);
    }
}
