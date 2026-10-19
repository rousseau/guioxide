//! État global et gabarit commun des écrans : en-tête, onglets, panneaux, barre de pied (`docs/interface.md`).

use std::path::PathBuf;

use egui::{Align, FontFamily, FontId, Key, KeyboardShortcut, Layout, Modifiers, RichText, Ui};
use guioxide_bids::{CheminsSession, Etape, EtatEtape};
use guioxide_geom::PointMonde;
use guioxide_view::{pastille_etat, polices, theme};

use crate::etapes::Visualisation;
use crate::ouverture::{self, Action, Chargement, EcranOuverture, JeuOuvert, Recent, Sortie};

/// Clé des jeux récents dans le stockage d'eframe.
const CLE_RECENTS: &str = "recents";

/// Un onglet : l'écran d'ouverture ou une étape de la chaîne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Onglet {
    Ouverture,
    Etape(Etape),
}

pub struct Application {
    ouvert: Option<JeuOuvert>,
    chargement: Option<Chargement>,
    /// Dernière erreur d'ouverture, affichée jusqu'à la prochaine tentative.
    erreur: Option<String>,
    sortie: Sortie,
    /// Indice de la session choisie dans `ouvert.lignes`.
    selection: Option<usize>,
    onglet: Onglet,
    ecran_ouverture: EcranOuverture,
    recents: Vec<Recent>,
    /// Curseur partagé par toutes les vues (monde, RAS, mm).
    curseur: PointMonde,
    visualisation: Visualisation,
}

impl Application {
    pub fn new(cc: &eframe::CreationContext<'_>, jeu: Option<PathBuf>) -> Self {
        let recents = cc
            .storage
            .and_then(|s| eframe::get_value(s, CLE_RECENTS))
            .unwrap_or_default();
        let mut app = Self {
            ouvert: None,
            chargement: None,
            erreur: None,
            sortie: Sortie::Etude,
            selection: None,
            onglet: Onglet::Ouverture,
            ecran_ouverture: EcranOuverture::default(),
            recents,
            curseur: PointMonde::default(),
            visualisation: Visualisation::new(&cc.egui_ctx),
        };
        if let Some(chemin) = jeu {
            app.ouvrir(&cc.egui_ctx, chemin, Sortie::Etude);
        }
        app
    }

    fn ouvrir(&mut self, ctx: &egui::Context, chemin: PathBuf, sortie: Sortie) {
        self.erreur = None;
        self.chargement = Some(Chargement::ouvrir(ctx, chemin, sortie.clone()));
        self.sortie = sortie;
    }

    fn choisir_dossier(&mut self, ctx: &egui::Context) {
        if let Some(dossier) = rfd::FileDialog::new().set_title("Ouvrir un jeu BIDS").pick_folder() {
            self.ouvrir(ctx, dossier, Sortie::Etude);
        }
    }

    /// Reçoit le résultat d'un chargement terminé.
    fn suivre_chargement(&mut self) {
        let Some(resultat) = self.chargement.as_ref().and_then(Chargement::resultat) else {
            return;
        };
        self.chargement = None;
        match resultat {
            Ok(ouvert) => {
                let meme_jeu = self.ouvert.as_ref().is_some_and(|o| o.jeu.racine == ouvert.jeu.racine);
                if !meme_jeu {
                    self.selection = (ouvert.lignes.len() == 1).then_some(0);
                    self.onglet = Onglet::Ouverture;
                }
                let chemin = ouverture::chemin_recent(&ouvert.jeu).to_path_buf();
                ouverture::memoriser(
                    &mut self.recents,
                    Recent {
                        chemin,
                        sortie: self.sortie.clone(),
                    },
                );
                self.ouvert = Some(ouvert);
            }
            Err(message) => self.erreur = Some(message),
        }
    }

    fn executer(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Ouvrir(chemin, sortie) => self.ouvrir(ctx, chemin, sortie),
            Action::ChoisirDossier => self.choisir_dossier(ctx),
            Action::ChangerSortie(sortie) => {
                if let Some(ouvert) = &self.ouvert {
                    self.chargement = Some(Chargement::recalculer(ctx, ouvert.jeu.clone(), sortie.clone()));
                }
                self.sortie = sortie;
            }
            Action::OuvrirSession => {
                if self.selection.is_some() {
                    self.onglet = Onglet::Etape(Etape::Examen);
                }
            }
        }
    }

    fn etat(&self, etape: Etape) -> Option<&EtatEtape> {
        let ligne = self.ouvert.as_ref()?.lignes.get(self.selection?)?;
        Some(&ligne.etats[usize::from(etape.numero() - 1)])
    }

    /// `1`…`4` changent d'onglet ; `Ctrl/Cmd + O` ouvre un jeu ; un dossier déposé est ouvert.
    fn raccourcis(&mut self, ctx: &egui::Context) {
        if let Some(chemin) = ouverture::depot(ctx) {
            self.ouvrir(ctx, chemin, Sortie::Etude);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O))) {
            self.choisir_dossier(ctx);
        }
        if ctx.memory(|m| m.focused().is_some()) {
            return; // un champ de saisie a le focus : les chiffres lui reviennent
        }
        let touches = [(Key::Num1, 1), (Key::Num2, 2), (Key::Num3, 3), (Key::Num4, 4)];
        for (touche, n) in touches {
            if ctx.input(|i| i.key_pressed(touche))
                && let Some(etape) = Etape::depuis_numero(n)
            {
                self.onglet = Onglet::Etape(etape);
            }
        }
    }

    fn en_tete(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("guioxide").font(FontId::new(16.0, FontFamily::Name(polices::SEMI_GRAS.into()))));
            ui.separator();
            match &self.ouvert {
                Some(ouvert) => {
                    ui.label(&ouvert.jeu.nom);
                    if let Some(ligne) = self.selection.and_then(|i| ouvert.lignes.get(i)) {
                        ui.label(RichText::new("/").color(theme::TEXTE_SECONDAIRE));
                        ui.strong(ouvert.libelle(ligne));
                        if let Some(age) = ouvert.age(ligne) {
                            ui.label(RichText::new(ouverture::format_age(age)).color(theme::TERMINE));
                        }
                        let session = &ouvert.jeu.sujets[ligne.sujet].sessions[ligne.session];
                        let contrastes = session.contrastes().into_iter().collect::<Vec<_>>().join(" ");
                        ui.label(RichText::new(format!("Contrastes : {contrastes}")).color(theme::TEXTE_SECONDAIRE));
                    }
                }
                None => {
                    ui.label(RichText::new("aucun jeu ouvert").color(theme::TEXTE_SECONDAIRE));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let raccourci = ui
                    .ctx()
                    .format_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O));
                if ui.button("Ouvrir un jeu BIDS").on_hover_text(raccourci).clicked() {
                    self.choisir_dossier(ui.ctx());
                }
                let pastille = match self.sortie {
                    Sortie::Etude => "Sortie : étude",
                    Sortie::Libre(_) => "Sortie : test",
                };
                if ui
                    .button(pastille)
                    .on_hover_text("Choisir le répertoire de sortie")
                    .clicked()
                {
                    self.onglet = Onglet::Ouverture;
                }
            });
        });
    }

    fn barre_onglets(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.onglet, Onglet::Ouverture, "Ouverture");
            for etape in Etape::TOUTES {
                ui.separator();
                pastille_etat(ui, self.etat(etape).unwrap_or(&EtatEtape::NonFaite));
                ui.selectable_value(&mut self.onglet, Onglet::Etape(etape), etape.to_string())
                    .on_hover_text(format!("{} · touche {}", etape.outil(), etape.numero()));
            }
        });
    }

    fn barre_pied(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let outil = match self.onglet {
                Onglet::Ouverture => concat!("guioxide ", env!("CARGO_PKG_VERSION")),
                Onglet::Etape(etape) => etape.outil(),
            };
            ui.label(RichText::new(outil).monospace().color(theme::TEXTE_SECONDAIRE));
            ui.separator();
            let derivees = match &self.ouvert {
                Some(o) => self.sortie.derivees(&o.jeu).display().to_string(),
                None => "aucune dérivée affichée".to_owned(),
            };
            ui.label(RichText::new(derivees).monospace().color(theme::TEXTE_SECONDAIRE));
            if let Some(chargement) = &self.chargement {
                ui.separator();
                ui.spinner();
                ui.label(RichText::new(&chargement.description).color(theme::TEXTE_SECONDAIRE));
            }
        });
    }

    fn ecran_ouverture(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if let Some(erreur) = &self.erreur {
            egui::Panel::top("erreur_ouverture").show(ui, |ui| {
                ui.label(RichText::new(format!("⚠ {erreur}")).color(theme::ERREUR));
            });
        }
        match &self.ouvert {
            Some(ouvert) => self
                .ecran_ouverture
                .jeu(ui, ouvert, &self.sortie, &mut self.selection, actions),
            None => {
                let survole = ouverture::depot_survole(ui.ctx());
                egui::CentralPanel::default()
                    .show(ui, |ui| self.ecran_ouverture.vide(ui, &self.recents, survole, actions));
            }
        }
    }

    /// Onglets 1 à 4 : écran de l'étape pour la session choisie, ou gabarit vide.
    fn ecran_etape(&mut self, ui: &mut Ui, etape: Etape) {
        let session = self
            .ouvert
            .as_ref()
            .zip(self.selection)
            .and_then(|(o, i)| Some((o, o.lignes.get(i)?, i)));
        if let Some((ouvert, ligne, i)) = session
            && matches!(etape, Etape::Examen | Etape::Reconstruction)
        {
            let derivees = self.sortie.derivees(&ouvert.jeu);
            let sujet = &ouvert.jeu.sujets[ligne.sujet];
            let session = &sujet.sessions[ligne.session];
            let chemins = CheminsSession::new(&derivees, sujet, session);
            self.visualisation.suivre(&derivees, i);
            match etape {
                Etape::Examen => self.visualisation.examen(ui, session, &chemins, &mut self.curseur),
                _ => self
                    .visualisation
                    .reconstruction(ui, session, &chemins, &mut self.curseur),
            }
            return;
        }
        self.gabarit_vide(ui, etape);
    }

    /// Gabarit des onglets 1 à 4 : données de l'étape à gauche, visualiseur au centre, qualité à droite.
    fn gabarit_vide(&self, ui: &mut Ui, etape: Etape) {
        egui::Panel::left(("donnees", etape.numero()))
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                ui.strong("Données de l'étape");
                ui.label(RichText::new(format!("fichiers {}", etape.outil())).color(theme::TEXTE_SECONDAIRE));
            });
        egui::Panel::right(("qualite", etape.numero()))
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                ui.strong("Qualité et paramètres");
                if let Some(EtatEtape::Erreur(message)) = self.etat(etape) {
                    ui.label(RichText::new(message).color(theme::ERREUR));
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                let texte = match self.selection {
                    Some(_) => format!("Visualiseur · {etape}"),
                    None => "Choisir une session sur l'écran d'ouverture".to_owned(),
                };
                ui.label(RichText::new(texte).color(theme::TEXTE_SECONDAIRE));
            });
        });
    }
}

impl eframe::App for Application {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.suivre_chargement();
        self.raccourcis(&ctx);
        egui::Panel::top("en_tete").show(ui, |ui| {
            ui.add_space(4.0);
            self.en_tete(ui);
            ui.add_space(2.0);
        });
        egui::Panel::top("onglets").show(ui, |ui| self.barre_onglets(ui));
        egui::Panel::bottom("pied").show(ui, |ui| self.barre_pied(ui));
        let mut actions = Vec::new();
        match self.onglet {
            Onglet::Ouverture => self.ecran_ouverture(ui, &mut actions),
            Onglet::Etape(etape) => self.ecran_etape(ui, etape),
        }
        for action in actions {
            self.executer(&ctx, action);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, CLE_RECENTS, &self.recents);
    }
}
