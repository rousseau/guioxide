//! État global et gabarit commun des écrans : en-tête, onglets, panneaux, barre de pied (`docs/interface.md`).

use std::path::PathBuf;

use egui::{Align, FontFamily, FontId, Key, Layout, RichText, Ui};
use guioxide_bids::{Etape, EtatEtape};
use guioxide_geom::PointMonde;
use guioxide_view::{pastille_etat, polices, theme};

/// Un onglet : l'écran d'ouverture ou une étape de la chaîne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Onglet {
    Ouverture,
    Etape(Etape),
}

/// Répertoire où sont lues les dérivées.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sortie {
    /// `racine/derivatives/<pipeline>/` du jeu ouvert.
    Etude,
    /// Répertoire libre, pour les essais ; choisi sur l'écran d'ouverture au jalon 2.
    #[allow(dead_code)]
    Libre(PathBuf),
}

pub struct Application {
    /// Chemin demandé à l'ouverture ; l'indexation BIDS arrive au jalon 2.
    jeu: Option<PathBuf>,
    sortie: Sortie,
    onglet: Onglet,
    /// Curseur partagé par toutes les vues d'un onglet ; utilisé à partir du jalon 3.
    #[allow(dead_code)]
    curseur: PointMonde,
    etats: [EtatEtape; 4],
}

impl Application {
    pub fn new(jeu: Option<PathBuf>) -> Self {
        Self {
            jeu,
            sortie: Sortie::Etude,
            onglet: Onglet::Ouverture,
            curseur: PointMonde::default(),
            etats: Default::default(),
        }
    }

    fn etat(&self, etape: Etape) -> &EtatEtape {
        &self.etats[usize::from(etape.numero() - 1)]
    }

    /// Raccourcis : `1`…`4` changent d'onglet. `Ctrl/Cmd + O` (ouvrir un jeu) arrive au jalon 2.
    fn raccourcis(&mut self, ui: &Ui) {
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return; // un champ de saisie a le focus : les chiffres lui reviennent
        }
        let touches = [(Key::Num1, 1), (Key::Num2, 2), (Key::Num3, 3), (Key::Num4, 4)];
        for (touche, n) in touches {
            if ui.input(|i| i.key_pressed(touche))
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
            match &self.jeu {
                Some(chemin) => ui.label(chemin.display().to_string()),
                None => ui.label(RichText::new("aucun jeu ouvert").color(theme::TEXTE_SECONDAIRE)),
            };
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled(false, egui::Button::new("Ouvrir un jeu BIDS"))
                    .on_disabled_hover_text("jalon 2");
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
                pastille_etat(ui, self.etat(etape));
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
            ui.label(RichText::new("aucune dérivée affichée").color(theme::TEXTE_SECONDAIRE));
        });
    }

    fn ecran_ouverture(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            egui::Frame::new()
                .stroke((1.0, theme::BORDURE))
                .corner_radius(8.0)
                .inner_margin(32.0)
                .show(ui, |ui| {
                    ui.set_width(480.0);
                    ui.vertical_centered(|ui| {
                        ui.heading("Ouvrir un jeu BIDS");
                        ui.label(
                            RichText::new("Glisser un dossier ici, ou cliquer pour le choisir (jalon 2)")
                                .color(theme::TEXTE_SECONDAIRE),
                        );
                    });
                });
        });
    }

    /// Gabarit des onglets 1 à 4 : données de l'étape à gauche, visualiseur au centre, qualité à droite.
    fn ecran_etape(&self, ui: &mut Ui, etape: Etape) {
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
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(format!("Visualiseur · {etape}")).color(theme::TEXTE_SECONDAIRE));
            });
        });
    }
}

impl eframe::App for Application {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.raccourcis(ui);
        egui::Panel::top("en_tete").show(ui, |ui| {
            ui.add_space(4.0);
            self.en_tete(ui);
            ui.add_space(2.0);
        });
        egui::Panel::top("onglets").show(ui, |ui| self.barre_onglets(ui));
        egui::Panel::bottom("pied").show(ui, |ui| self.barre_pied(ui));
        match self.onglet {
            Onglet::Ouverture => {
                egui::CentralPanel::default().show(ui, |ui| self.ecran_ouverture(ui));
            }
            Onglet::Etape(etape) => self.ecran_etape(ui, etape),
        }
    }
}
