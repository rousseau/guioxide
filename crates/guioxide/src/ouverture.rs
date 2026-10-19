//! Écran d'ouverture : chargement d'un jeu BIDS en arrière-plan, tableau sujets × sessions, choix de la sortie.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use egui::{Align, Layout, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use guioxide_bids::{EtatEtape, JeuBids, etats_session};
use guioxide_view::{pastille_etat, theme};
use serde::{Deserialize, Serialize};

/// Répertoire où sont lues les dérivées.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sortie {
    /// `derivatives/` de l'étude.
    Etude,
    /// Répertoire libre, pour les essais : on y lit `<chemin>/derivatives/`.
    Libre(PathBuf),
}

impl Sortie {
    pub fn derivees(&self, jeu: &JeuBids) -> PathBuf {
        match self {
            Sortie::Etude => jeu.derivees_etude(),
            Sortie::Libre(chemin) => chemin.join("derivatives"),
        }
    }
}

/// Un jeu ouvert : l'index, et l'état des quatre étapes pour chaque session, dans la sortie choisie.
pub struct JeuOuvert {
    pub jeu: JeuBids,
    pub lignes: Vec<Ligne>,
}

/// Une session du tableau.
pub struct Ligne {
    pub sujet: usize,
    pub session: usize,
    pub etats: [EtatEtape; 4],
}

impl JeuOuvert {
    fn calculer(jeu: JeuBids, sortie: &Sortie) -> JeuOuvert {
        let derivees = sortie.derivees(&jeu);
        let mut lignes = Vec::new();
        for (i, sujet) in jeu.sujets.iter().enumerate() {
            for (j, session) in sujet.sessions.iter().enumerate() {
                lignes.push(Ligne {
                    sujet: i,
                    session: j,
                    etats: etats_session(&derivees, sujet, session),
                });
            }
        }
        JeuOuvert { jeu, lignes }
    }

    /// `sub-X / ses-Y` (ou `sub-X` sans session).
    pub fn libelle(&self, ligne: &Ligne) -> String {
        let sujet = &self.jeu.sujets[ligne.sujet];
        match &sujet.sessions[ligne.session].id {
            Some(ses) => format!("{} / {ses}", sujet.id),
            None => sujet.id.clone(),
        }
    }

    pub fn age(&self, ligne: &Ligne) -> Option<f64> {
        self.jeu.sujets[ligne.sujet].sessions[ligne.session].age_gestationnel
    }
}

/// Âge gestationnel en semaines d'aménorrhée, virgule décimale.
pub fn format_age(age: f64) -> String {
    format!("{age:.1} SA").replace('.', ",")
}

/// Un jeu ouvert récemment, avec la sortie utilisée.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recent {
    pub chemin: PathBuf,
    pub sortie: Sortie,
}

/// Indexation ou recalcul des états, sur un fil d'arrière-plan.
pub struct Chargement {
    pub description: String,
    recepteur: mpsc::Receiver<Result<JeuOuvert, String>>,
}

impl Chargement {
    /// Indexe le jeu qui contient `chemin`.
    pub fn ouvrir(ctx: &egui::Context, chemin: PathBuf, sortie: Sortie) -> Chargement {
        let description = format!("Indexation de {}", chemin.display());
        Chargement::lancer(ctx, description, move || {
            let jeu = guioxide_bids::indexer(&chemin).map_err(|e| e.to_string())?;
            Ok(JeuOuvert::calculer(jeu, &sortie))
        })
    }

    /// Recalcule les états d'un jeu déjà indexé pour une autre sortie.
    pub fn recalculer(ctx: &egui::Context, jeu: JeuBids, sortie: Sortie) -> Chargement {
        Chargement::lancer(ctx, "Lecture des dérivées".to_owned(), move || {
            Ok(JeuOuvert::calculer(jeu, &sortie))
        })
    }

    fn lancer(
        ctx: &egui::Context,
        description: String,
        travail: impl FnOnce() -> Result<JeuOuvert, String> + Send + 'static,
    ) -> Chargement {
        let (emetteur, recepteur) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = emetteur.send(travail());
            ctx.request_repaint();
        });
        Chargement { description, recepteur }
    }

    /// Résultat, s'il est arrivé.
    pub fn resultat(&self) -> Option<Result<JeuOuvert, String>> {
        match self.recepteur.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("le fil d'indexation s'est arrêté".to_owned())),
        }
    }
}

/// Ce que l'utilisateur a demandé sur l'écran d'ouverture ; l'application l'exécute.
pub enum Action {
    Ouvrir(PathBuf, Sortie),
    ChoisirDossier,
    ChangerSortie(Sortie),
    OuvrirSession,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tri {
    Session,
    Age,
}

/// État propre à l'écran : filtres, tri, saisie du répertoire libre.
pub struct EcranOuverture {
    recherche: String,
    contraste: Option<String>,
    non_termine_seulement: bool,
    tri: Tri,
    croissant: bool,
    saisie_libre: String,
}

impl Default for EcranOuverture {
    fn default() -> Self {
        EcranOuverture {
            recherche: String::new(),
            contraste: None,
            non_termine_seulement: false,
            tri: Tri::Session,
            croissant: true,
            saisie_libre: String::new(),
        }
    }
}

impl EcranOuverture {
    /// État vide : zone de dépôt et jeux récents.
    pub fn vide(&self, ui: &mut Ui, recents: &[Recent], depot_survole: bool, actions: &mut Vec<Action>) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.25);
            let bordure = if depot_survole { theme::ACCENT } else { theme::BORDURE };
            let zone = egui::Frame::new()
                .stroke((1.0, bordure))
                .corner_radius(8.0)
                .inner_margin(32.0)
                .show(ui, |ui| {
                    ui.set_width(480.0);
                    ui.vertical_centered(|ui| {
                        ui.heading("Ouvrir un jeu BIDS");
                        ui.label(
                            RichText::new("Glisser un dossier ici, ou cliquer pour le choisir")
                                .color(theme::TEXTE_SECONDAIRE),
                        );
                    });
                });
            if zone
                .response
                .interact(Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                actions.push(Action::ChoisirDossier);
            }
            if !recents.is_empty() {
                ui.add_space(24.0);
                ui.label(RichText::new("Récents").color(theme::TEXTE_SECONDAIRE));
                for recent in recents {
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 480.0).max(0.0) / 2.0);
                        let badge = match recent.sortie {
                            Sortie::Etude => "étude",
                            Sortie::Libre(_) => "test",
                        };
                        ui.label(RichText::new(badge).small().color(theme::TEXTE_SECONDAIRE));
                        if ui.link(recent.chemin.display().to_string()).clicked() {
                            actions.push(Action::Ouvrir(recent.chemin.clone(), recent.sortie.clone()));
                        }
                    });
                }
            }
        });
    }

    /// Jeu chargé : informations à gauche, sortie à droite, tableau au centre.
    pub fn jeu(
        &mut self,
        ui: &mut Ui,
        ouvert: &JeuOuvert,
        sortie: &Sortie,
        selection: &mut Option<usize>,
        actions: &mut Vec<Action>,
    ) {
        egui::Panel::left("ouverture_jeu")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| self.panneau_jeu(ui, ouvert));
        egui::Panel::right("ouverture_sortie")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| self.panneau_sortie(ui, &ouvert.jeu, sortie, actions));
        egui::CentralPanel::default().show(ui, |ui| {
            egui::Panel::bottom("ouverture_action")
                .show_separator_line(false)
                .show(ui, |ui| {
                    ui.add_space(6.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let ligne = selection.and_then(|i| ouvert.lignes.get(i));
                        let texte = match ligne {
                            Some(l) => format!("Ouvrir {} →", ouvert.libelle(l)),
                            None => "Choisir une session".to_owned(),
                        };
                        if ui.add_enabled(ligne.is_some(), egui::Button::new(texte)).clicked() {
                            actions.push(Action::OuvrirSession);
                        }
                    });
                });
            self.tableau(ui, ouvert, selection, actions);
        });
    }

    fn panneau_jeu(&mut self, ui: &mut Ui, ouvert: &JeuOuvert) {
        let jeu = &ouvert.jeu;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(&jeu.nom);
            ui.label(
                RichText::new(jeu.racine.display().to_string())
                    .small()
                    .monospace()
                    .color(theme::TEXTE_SECONDAIRE),
            );
            ui.add_space(6.0);
            egui::Grid::new("infos_jeu").num_columns(2).show(ui, |ui| {
                ui.label("BIDS");
                ui.label(jeu.version_bids.as_deref().unwrap_or("version non indiquée"));
                ui.end_row();
                ui.label("Sujets");
                ui.label(jeu.sujets.len().to_string());
                ui.end_row();
                ui.label("Sessions");
                ui.label(jeu.nb_sessions().to_string());
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.strong("Dérivées de l'étude");
            if jeu.derivees.is_empty() {
                ui.label(RichText::new("aucune").color(theme::TEXTE_SECONDAIRE));
            }
            for d in &jeu.derivees {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&d.pipeline).monospace());
                    ui.label(
                        RichText::new(d.generateur.as_deref().unwrap_or("version inconnue"))
                            .color(theme::TEXTE_SECONDAIRE),
                    );
                });
            }
            if !jeu.avertissements.is_empty() {
                ui.add_space(8.0);
                ui.strong(format!("Avertissements ({})", jeu.avertissements.len()));
                for a in &jeu.avertissements {
                    ui.label(RichText::new(format!("⚠ {a}")).color(theme::ACCENT));
                }
            }
            ui.add_space(8.0);
            ui.separator();
            ui.strong("Filtres");
            egui::ComboBox::from_label("Contraste")
                .selected_text(self.contraste.as_deref().unwrap_or("tous"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.contraste, None, "tous");
                    for c in jeu.contrastes() {
                        ui.selectable_value(&mut self.contraste, Some(c.to_owned()), c);
                    }
                });
            ui.checkbox(&mut self.non_termine_seulement, "Non terminé seulement");
        });
    }

    fn panneau_sortie(&mut self, ui: &mut Ui, jeu: &JeuBids, sortie: &Sortie, actions: &mut Vec<Action>) {
        ui.strong("Répertoire de sortie");
        ui.add_space(4.0);
        if ui.radio(*sortie == Sortie::Etude, "Étude — dérivées BIDS").clicked() && *sortie != Sortie::Etude {
            actions.push(Action::ChangerSortie(Sortie::Etude));
        }
        ui.label(
            RichText::new(jeu.derivees_etude().display().to_string())
                .small()
                .monospace()
                .color(theme::TEXTE_SECONDAIRE),
        );
        ui.add_space(8.0);
        let libre = matches!(sortie, Sortie::Libre(_));
        if ui.radio(libre, "Répertoire libre — tests").clicked() && !libre && !self.saisie_libre.trim().is_empty() {
            actions.push(Action::ChangerSortie(Sortie::Libre(PathBuf::from(
                self.saisie_libre.trim(),
            ))));
        }
        if let Sortie::Libre(chemin) = sortie
            && self.saisie_libre.is_empty()
        {
            self.saisie_libre = chemin.display().to_string();
        }
        ui.horizontal(|ui| {
            let champ = ui.add(
                egui::TextEdit::singleline(&mut self.saisie_libre)
                    .hint_text("chemin")
                    .desired_width(180.0),
            );
            let valide = champ.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Parcourir…").clicked()
                && let Some(dossier) = rfd::FileDialog::new().pick_folder()
            {
                self.saisie_libre = dossier.display().to_string();
                actions.push(Action::ChangerSortie(Sortie::Libre(dossier)));
            } else if valide && !self.saisie_libre.trim().is_empty() {
                actions.push(Action::ChangerSortie(Sortie::Libre(PathBuf::from(
                    self.saisie_libre.trim(),
                ))));
            }
        });
        if libre {
            ui.label(
                RichText::new("Les états du tableau sont ceux de ce répertoire, pas de l'étude.")
                    .small()
                    .color(theme::TEXTE_SECONDAIRE),
            );
        }
    }

    fn lignes_visibles(&self, ouvert: &JeuOuvert) -> Vec<usize> {
        let recherche = self.recherche.to_lowercase();
        let mut visibles: Vec<usize> = (0..ouvert.lignes.len())
            .filter(|&i| {
                let l = &ouvert.lignes[i];
                let session = &ouvert.jeu.sujets[l.sujet].sessions[l.session];
                (recherche.is_empty() || ouvert.libelle(l).to_lowercase().contains(&recherche))
                    && self
                        .contraste
                        .as_ref()
                        .is_none_or(|c| session.contrastes().contains(c.as_str()))
                    && (!self.non_termine_seulement || l.etats.iter().any(|e| *e != EtatEtape::Terminee))
            })
            .collect();
        if self.tri == Tri::Age {
            // Les sessions sans âge restent en fin de liste, dans les deux sens.
            let age = |i: usize| ouvert.age(&ouvert.lignes[i]);
            let (mut datees, sans_age): (Vec<usize>, Vec<usize>) =
                visibles.into_iter().partition(|&i| age(i).is_some());
            datees.sort_by(|&a, &b| age(a).unwrap_or_default().total_cmp(&age(b).unwrap_or_default()));
            if !self.croissant {
                datees.reverse();
            }
            datees.extend(sans_age);
            return datees;
        }
        if !self.croissant {
            visibles.reverse();
        }
        visibles
    }

    fn tableau(&mut self, ui: &mut Ui, ouvert: &JeuOuvert, selection: &mut Option<usize>, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.recherche).hint_text("Rechercher un sujet ou une session"));
        });
        ui.add_space(4.0);
        let visibles = self.lignes_visibles(ouvert);
        let hauteur = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
        TableBuilder::new(ui)
            .striped(true)
            .sense(Sense::click())
            .column(Column::initial(200.0).at_least(120.0))
            .column(Column::initial(80.0))
            .column(Column::initial(120.0))
            .columns(Column::exact(32.0), 4)
            .column(Column::remainder())
            .header(hauteur, |mut entete| {
                entete.col(|ui| self.entete_triable(ui, "Session", Tri::Session));
                entete.col(|ui| self.entete_triable(ui, "AG", Tri::Age));
                entete.col(|ui| {
                    ui.strong("Contrastes");
                });
                for n in 1..=4 {
                    entete.col(|ui| {
                        ui.strong(n.to_string());
                    });
                }
                entete.col(|_| {});
            })
            .body(|corps| {
                corps.rows(hauteur, visibles.len(), |mut rangee| {
                    let i = visibles[rangee.index()];
                    let ligne = &ouvert.lignes[i];
                    let session = &ouvert.jeu.sujets[ligne.sujet].sessions[ligne.session];
                    rangee.set_selected(*selection == Some(i));
                    rangee.col(|ui| {
                        ui.label(ouvert.libelle(ligne));
                    });
                    rangee.col(|ui| {
                        ui.label(ouvert.age(ligne).map(format_age).unwrap_or_else(|| "—".to_owned()));
                    });
                    rangee.col(|ui| {
                        ui.label(session.contrastes().into_iter().collect::<Vec<_>>().join(" "));
                    });
                    for etat in &ligne.etats {
                        rangee.col(|ui| pastille_etat(ui, etat));
                    }
                    rangee.col(|_| {});
                    let reponse = rangee.response();
                    if reponse.clicked() {
                        *selection = Some(i);
                    }
                    if reponse.double_clicked() {
                        actions.push(Action::OuvrirSession);
                    }
                });
            });
    }

    fn entete_triable(&mut self, ui: &mut Ui, titre: &str, tri: Tri) {
        let fleche = match (self.tri == tri, self.croissant) {
            (false, _) => "",
            (true, true) => " ⏶",
            (true, false) => " ⏷",
        };
        if ui
            .add(egui::Button::new(RichText::new(format!("{titre}{fleche}")).strong()).frame(false))
            .clicked()
        {
            if self.tri == tri {
                self.croissant = !self.croissant;
            } else {
                (self.tri, self.croissant) = (tri, true);
            }
        }
    }
}

/// Premier dossier (ou fichier) déposé dans la fenêtre pendant cette image.
pub fn depot(ctx: &egui::Context) -> Option<PathBuf> {
    ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path())
            .find(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
    })
}

/// Vrai si l'utilisateur survole la fenêtre avec un fichier.
pub fn depot_survole(ctx: &egui::Context) -> bool {
    ctx.input(|i| !i.raw.hovered_files.is_empty())
}

/// Ajoute `recent` en tête de liste (sans doublon), dix au plus.
pub fn memoriser(recents: &mut Vec<Recent>, recent: Recent) {
    recents.retain(|r| r.chemin != recent.chemin);
    recents.insert(0, recent);
    recents.truncate(10);
}

/// Racine à mémoriser : celle de l'étude, pour retrouver le même jeu quel que soit le dossier déposé.
pub fn chemin_recent(jeu: &JeuBids) -> &Path {
    &jeu.racine_etude
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_en_semaines_avec_virgule() {
        assert_eq!(format_age(30.5), "30,5 SA");
        assert_eq!(format_age(35.71), "35,7 SA");
    }

    #[test]
    fn recents_sans_doublon_et_bornes() {
        let r = |n: usize| Recent {
            chemin: PathBuf::from(format!("/j{n}")),
            sortie: Sortie::Etude,
        };
        let mut recents = Vec::new();
        for n in 0..12 {
            memoriser(&mut recents, r(n));
        }
        memoriser(&mut recents, r(5));
        assert_eq!(recents.len(), 10);
        assert_eq!(recents[0], r(5));
        assert_eq!(recents.iter().filter(|x| **x == r(5)).count(), 1);
    }
}
