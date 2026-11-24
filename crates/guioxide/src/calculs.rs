//! Fenêtre « Calculs » d'une séance : chaque étape de la chaîne se lance seule, ou une sélection d'étapes dans
//! l'ordre ; réglages de la reconstruction (résolution, mouvement, piles utilisées…) ; suivi, temps estimé d'après
//! les exécutions précédentes sur cette machine, annulation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::{Color32, RichText, Ui};
use guioxide_bids::{CheminsSession, Session};
use guioxide_view::theme;
use serde::{Deserialize, Serialize};

use crate::chaine::{self, Chaine, Etape, EtatEtape, Outil};

/// Les étapes de la chaîne, dans l'ordre où elles dépendent les unes des autres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Calcul {
    Masques,
    Controle,
    Reconstruction,
    Segmentation,
    Surfaces,
    Maillage,
    Simulation,
}

impl Calcul {
    pub const TOUS: [Calcul; 7] = [
        Calcul::Masques,
        Calcul::Controle,
        Calcul::Reconstruction,
        Calcul::Segmentation,
        Calcul::Surfaces,
        Calcul::Maillage,
        Calcul::Simulation,
    ];

    pub fn libelle(self) -> &'static str {
        match self {
            Calcul::Masques => "Masques du cerveau",
            Calcul::Controle => "Contrôle des piles",
            Calcul::Reconstruction => "Reconstruction (SVR)",
            Calcul::Segmentation => "Segmentation",
            Calcul::Surfaces => "Surfaces corticales",
            Calcul::Maillage => "Maillage",
            Calcul::Simulation => "Simulation du plissement",
        }
    }

    fn commande(self) -> &'static str {
        match self {
            Calcul::Masques => "medx fetalbet",
            Calcul::Controle => "medx svr qc",
            Calcul::Reconstruction => "medx svr recon",
            Calcul::Segmentation => "medx bounti",
            Calcul::Surfaces => "surfx run",
            Calcul::Maillage => "gyroxide mesh",
            Calcul::Simulation => "gyroxide run",
        }
    }

    fn outil(self) -> Outil {
        match self {
            Calcul::Masques | Calcul::Controle | Calcul::Reconstruction | Calcul::Segmentation => Outil::Medx,
            Calcul::Surfaces => Outil::Surfx,
            Calcul::Maillage | Calcul::Simulation => Outil::Gyroxide,
        }
    }

    /// Ce qu'il faut avoir calculé avant.
    fn prealable(self) -> Option<Calcul> {
        match self {
            Calcul::Masques => None,
            Calcul::Controle | Calcul::Reconstruction => Some(Calcul::Masques),
            Calcul::Segmentation => Some(Calcul::Reconstruction),
            Calcul::Surfaces => Some(Calcul::Segmentation),
            Calcul::Maillage => Some(Calcul::Surfaces),
            Calcul::Simulation => Some(Calcul::Maillage),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mouvement {
    S2s,
    S2v,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hemisphere {
    Gauche,
    Droit,
}

/// Réglages des étapes, mémorisés d'une séance à l'autre.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Reglages {
    /// Côté des voxels du volume reconstruit (mm).
    pub resolution: f64,
    pub mouvement: Mouvement,
    /// Poids de la régularisation.
    pub alpha: f64,
    /// Reconstruction sur le GPU (Metal sur Mac) plutôt que le CPU.
    pub gpu: bool,
    pub hemisphere: Hemisphere,
    pub pas: u32,
    pub selection: BTreeSet<Calcul>,
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages {
            resolution: 0.8,
            mouvement: Mouvement::S2s,
            alpha: 0.05,
            gpu: false,
            hemisphere: Hemisphere::Gauche,
            pas: 2000,
            selection: [Calcul::Masques, Calcul::Controle].into_iter().collect(),
        }
    }
}

/// Durée mémorisée d'une étape : secondes par unité de travail (pile, coupe, pas…).
pub type Durees = BTreeMap<String, f64>;

/// Mesures du 24 novembre 2026 sur un MacBook Pro (Apple Silicon), utilisées tant qu'aucune exécution n'a eu lieu
/// sur cette machine : fetalbet 13 à 24 s par pile ; svr qc 10 ms par coupe ; SVR s2s 8 s par coupe à 0,8 mm ; bounti 40 s ; surfx 80 s ;
/// gyroxide mesh 4 s ; gyroxide run 17 ms par pas.
fn duree_de_reference(cle: &str) -> Option<f64> {
    Some(match cle {
        "masques" => 13.0,
        "controle" => 0.01,
        "svr-s2s" => 8.2,
        "segmentation" => 40.0,
        "surfaces" => 80.0,
        "maillage" => 4.0,
        "simulation" => 0.017,
        _ => return None,
    })
}

/// Une séance telle que les outils la désignent.
#[derive(Clone, Debug, PartialEq)]
pub struct Seance {
    /// Racine BIDS (`--bids`).
    pub racine: PathBuf,
    /// Sans le préfixe `sub-`.
    pub participant: String,
    /// Sans le préfixe `ses-`.
    pub session: Option<String>,
    /// Racine d'étude d'une sortie libre (`--output-dir`), sinon celle de l'étude.
    pub sortie: Option<PathBuf>,
    pub libelle: String,
}

impl Seance {
    fn arguments(&self, sous_commandes: &[&str]) -> Vec<String> {
        let mut a: Vec<String> = sous_commandes.iter().map(|s| s.to_string()).collect();
        a.push("--bids".into());
        a.push(self.racine.display().to_string());
        a.push("--participant".into());
        a.push(self.participant.clone());
        if let Some(s) = &self.session {
            a.push("--session".into());
            a.push(s.clone());
        }
        if let Some(d) = &self.sortie {
            a.push("--output-dir".into());
            a.push(d.display().to_string());
        }
        a
    }
}

/// Fenêtre « Calculs » et chaîne en cours.
#[derive(Default)]
pub struct Calculs {
    pub ouverte: bool,
    pub reglages: Reglages,
    pub durees: Durees,
    /// Piles retirées de la reconstruction, par séance (nom de la pile).
    exclues: BTreeMap<String, BTreeSet<String>>,
    pub chaine: Option<(Chaine, Seance)>,
    /// Étapes dont la durée a déjà été mémorisée, pour la chaîne en cours.
    enregistrees: BTreeSet<usize>,
    pub suivi: bool,
    erreur: Option<String>,
    /// Nombre de coupes de chaque pile, lu une fois dans l'en-tête.
    coupes: BTreeMap<PathBuf, usize>,
}

impl Calculs {
    pub fn en_cours(&self) -> bool {
        self.chaine.as_ref().is_some_and(|(c, _)| !c.finie())
    }

    /// Vrai si une étape s'est terminée depuis le dernier appel : les dérivées sont à relire.
    pub fn a_relire(&self) -> bool {
        self.chaine.as_ref().is_some_and(|(c, _)| c.a_relire())
    }

    /// Fenêtres « Calculs » (si une séance est choisie) et « Suivi » ; mémorise la durée des étapes terminées.
    pub fn afficher(&mut self, ctx: &egui::Context, seance: Option<(&Seance, &Session, &CheminsSession)>) {
        self.memoriser_durees();
        if let Some((seance, session, chemins)) = seance {
            let mut ouverte = self.ouverte;
            egui::Window::new(format!("Calculs · {}", seance.libelle))
                .id(egui::Id::unique("fenetre_calculs"))
                .open(&mut ouverte)
                .default_width(560.0)
                .default_pos(ctx.content_rect().right_top() + egui::vec2(-600.0, 90.0))
                .show(ctx, |ui| self.formulaire(ui, seance, session, chemins));
            self.ouverte = ouverte;
        }
        self.fenetre_suivi(ctx);
    }

    fn memoriser_durees(&mut self) {
        let Some((chaine, _)) = &self.chaine else { return };
        for (i, etat) in chaine.etats().iter().enumerate() {
            if let EtatEtape::Terminee(d) = etat
                && self.enregistrees.insert(i)
            {
                let e = &chaine.etapes[i];
                self.durees.insert(e.cle.clone(), d.as_secs_f64() / e.unites.max(1e-9));
            }
        }
    }

    fn formulaire(&mut self, ui: &mut Ui, seance: &Seance, session: &Session, chemins: &CheminsSession) {
        let cle_seance = format!(
            "{}|{}|{:?}",
            seance.racine.display(),
            seance.participant,
            seance.session
        );
        let piles: Vec<&guioxide_bids::Pile> = session.piles.iter().filter(|p| p.contraste == "T2w").collect();
        let exclues = self.exclues.entry(cle_seance).or_default();
        let utilisees: Vec<&guioxide_bids::Pile> =
            piles.iter().copied().filter(|p| !exclues.contains(&p.nom)).collect();
        let cache = &mut self.coupes;
        let coupes: usize = utilisees
            .iter()
            .map(|p| {
                *cache
                    .entry(p.chemin.clone())
                    .or_insert_with(|| guioxide_data::lire_geometrie(&p.chemin).map_or(0, |(d, _)| d[2]))
            })
            .sum();
        let r = &mut self.reglages;
        let mut lancer: Option<Vec<Calcul>> = None;
        let occupe = self.chaine.as_ref().is_some_and(|(c, _)| !c.finie());

        ui.label(
            RichText::new("Cocher les étapes à enchaîner, ou lancer une étape seule. Chaque étape lit les sorties des précédentes.")
                .color(theme::TEXTE_SECONDAIRE),
        );
        ui.add_space(6.0);
        egui::Grid::new("calculs_etapes")
            .num_columns(5)
            .spacing([10.0, 6.0])
            .striped(true)
            .show(ui, |ui| {
                for c in Calcul::TOUS {
                    let mut coche = r.selection.contains(&c);
                    if ui.checkbox(&mut coche, "").changed() {
                        if coche {
                            r.selection.insert(c);
                        } else {
                            r.selection.remove(&c);
                        }
                    }
                    let fait = deja_fait(c, chemins, session, r.hemisphere);
                    ui.horizontal(|ui| {
                        let (signe, couleur) = if fait {
                            ("✓", theme::TERMINE)
                        } else {
                            ("○", theme::TEXTE_SECONDAIRE)
                        };
                        ui.label(RichText::new(signe).color(couleur)).on_hover_text(if fait {
                            "sorties présentes"
                        } else {
                            "pas encore calculé"
                        });
                        ui.label(c.libelle());
                    });
                    ui.label(
                        RichText::new(c.commande())
                            .monospace()
                            .small()
                            .color(theme::TEXTE_SECONDAIRE),
                    );
                    let (cle, unites) = travail(c, r, piles.len(), coupes);
                    let estimation = self_estimation(&self.durees, &cle, unites);
                    ui.label(
                        RichText::new(estimation.map_or("—".to_owned(), |d| format!("≈ {}", duree_lisible(d))))
                            .small()
                            .color(theme::TEXTE_SECONDAIRE),
                    )
                    .on_hover_text("estimation d'après la dernière exécution sur cette machine");
                    let manque = c.prealable().filter(|p| !deja_fait(*p, chemins, session, r.hemisphere));
                    let bouton = ui.add_enabled(!occupe, egui::Button::new("Lancer"));
                    let bouton = match manque {
                        Some(p) => bouton.on_hover_text(format!(
                            "les sorties de « {} » manquent : cocher les deux étapes et lancer la sélection",
                            p.libelle()
                        )),
                        None => bouton,
                    };
                    if bouton.clicked() {
                        lancer = Some(vec![c]);
                    }
                    ui.end_row();
                }
            });

        ui.add_space(8.0);
        egui::CollapsingHeader::new(RichText::new("Reconstruction").strong())
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("reglages_svr")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Résolution");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut r.resolution)
                                    .range(0.3..=2.0)
                                    .speed(0.01)
                                    .max_decimals(2)
                                    .suffix(" mm"),
                            );
                            for v in [0.5, 0.8, 1.0] {
                                if ui
                                    .selectable_label((r.resolution - v).abs() < 1e-9, format!("{v}").replace('.', ","))
                                    .clicked()
                                {
                                    r.resolution = v;
                                }
                            }
                        });
                        ui.end_row();
                        ui.label("Mouvement");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut r.mouvement, Mouvement::S2s, "coupe-coupe (s2s)");
                            ui.radio_value(&mut r.mouvement, Mouvement::S2v, "coupe-volume (s2v, plus long)");
                        });
                        ui.end_row();
                        ui.label("Régularisation");
                        ui.add(
                            egui::DragValue::new(&mut r.alpha)
                                .range(0.0..=1.0)
                                .speed(0.005)
                                .max_decimals(3),
                        );
                        ui.end_row();
                        ui.label("Calcul");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut r.gpu, false, "CPU");
                            ui.radio_value(&mut r.gpu, true, "GPU (Metal)").on_hover_text(
                                "medx : le chemin GPU de la SVR est aujourd'hui souvent plus lent que le CPU",
                            );
                        });
                        ui.end_row();
                    });
                ui.add_space(4.0);
                ui.label(format!(
                    "Piles utilisées ({} sur {}, {coupes} coupes)",
                    utilisees.len(),
                    piles.len()
                ));
                for p in &piles {
                    let mut garder = !exclues.contains(&p.nom);
                    let nom = crate::etapes::nom_court(&p.nom, chemins.prefixe()).to_owned();
                    if ui.checkbox(&mut garder, RichText::new(nom).monospace()).changed() {
                        if garder {
                            exclues.remove(&p.nom);
                        } else {
                            exclues.insert(p.nom.clone());
                        }
                    }
                }
                if utilisees.len() < 2 {
                    ui.label(RichText::new("deux piles au moins").color(theme::ERREUR));
                }
            });
        egui::CollapsingHeader::new(RichText::new("Simulation").strong()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Hémisphère");
                ui.radio_value(&mut r.hemisphere, Hemisphere::Gauche, "gauche");
                ui.radio_value(&mut r.hemisphere, Hemisphere::Droit, "droit");
                ui.separator();
                ui.add(egui::DragValue::new(&mut r.pas).range(10..=100_000).suffix(" pas"));
            });
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let selection: Vec<Calcul> = Calcul::TOUS.into_iter().filter(|c| r.selection.contains(c)).collect();
            let bouton =
                egui::Button::new(RichText::new(format!("Lancer la sélection ({})", selection.len())).strong())
                    .fill(theme::ACCENT.gamma_multiply(0.35));
            if ui.add_enabled(!occupe && !selection.is_empty(), bouton).clicked() {
                lancer = Some(selection);
            }
            if ui.small_button("tout").clicked() {
                r.selection = Calcul::TOUS.into_iter().collect();
            }
            if ui.small_button("rien").clicked() {
                r.selection.clear();
            }
            if occupe {
                ui.label(RichText::new("une chaîne est en cours").color(theme::TEXTE_SECONDAIRE));
            }
        });
        if let Some(e) = &self.erreur {
            ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
        }
        if let Some(calculs) = lancer {
            let utilisees: Vec<PathBuf> = utilisees.iter().map(|p| p.chemin.clone()).collect();
            let total = piles.len();
            match self.lancer(ui.ctx(), seance, chemins, &calculs, &utilisees, total, coupes) {
                Ok(()) => self.erreur = None,
                Err(e) => self.erreur = Some(e),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lancer(
        &mut self,
        ctx: &egui::Context,
        seance: &Seance,
        chemins: &CheminsSession,
        calculs: &[Calcul],
        piles: &[PathBuf],
        total_piles: usize,
        coupes: usize,
    ) -> Result<(), String> {
        let piles_toutes = piles.len() == total_piles;
        let r = &self.reglages;
        if calculs
            .iter()
            .any(|c| matches!(c, Calcul::Controle | Calcul::Reconstruction))
            && piles.len() < 2
        {
            return Err("deux piles au moins pour le contrôle et la reconstruction".into());
        }
        let mut etapes = Vec::new();
        for &c in calculs {
            let programme = chaine::trouver(c.outil()).ok_or(format!(
                "{} introuvable (PATH, ~/.cargo/bin, ~/Exp/oxide/…/target/release)",
                c.outil().binaire()
            ))?;
            let restreindre = |mut a: Vec<String>| {
                if !piles_toutes {
                    a.push("--input".into());
                    a.extend(piles.iter().map(|p| p.display().to_string()));
                }
                a
            };
            let hemi = match r.hemisphere {
                Hemisphere::Gauche => "L",
                Hemisphere::Droit => "R",
            };
            let arguments = match c {
                Calcul::Masques => seance.arguments(&["fetalbet"]),
                Calcul::Controle => restreindre(seance.arguments(&["svr", "qc"])),
                Calcul::Reconstruction => {
                    let mut a = restreindre(seance.arguments(&["svr", "recon"]));
                    let mouvement = match r.mouvement {
                        Mouvement::S2s => "s2s",
                        Mouvement::S2v => "s2v",
                    };
                    a.extend([
                        "--motion".into(),
                        mouvement.into(),
                        "--resolution-mm".into(),
                        format!("{}", r.resolution),
                        "--alpha".into(),
                        format!("{}", r.alpha),
                        "--device".into(),
                        if r.gpu { "wgpu" } else { "cpu" }.into(),
                    ]);
                    a
                }
                Calcul::Segmentation => seance.arguments(&["bounti"]),
                Calcul::Surfaces => seance.arguments(&["run"]),
                Calcul::Maillage => {
                    let mut a = seance.arguments(&["mesh"]);
                    a.extend(["--hemi".into(), hemi.into()]);
                    let tetgen = chaine::trouver_tetgen()
                        .ok_or("TetGen introuvable (gyroxide mesh) : GYROXIDE_TETGEN ou ~/.local/bin")?;
                    a.extend(["--tetgen".into(), tetgen.display().to_string()]);
                    a
                }
                Calcul::Simulation => {
                    let mut a = seance.arguments(&["run"]);
                    a.extend(
                        [
                            "--label",
                            "guioxide",
                            "--hemi",
                            hemi,
                            "--steps",
                            &r.pas.to_string(),
                            "--uniform-growth",
                            "--no-midplane",
                        ]
                        .map(str::to_owned),
                    );
                    a
                }
            };
            let mut etape = Etape::new(c.commande(), &programme, arguments);
            let (cle, unites) = travail(c, r, total_piles, coupes);
            etape.prevue = self_estimation(&self.durees, &cle, unites);
            (etape.cle, etape.unites) = (cle, unites);
            if c == Calcul::Simulation {
                etape.pas = Some(r.pas as u64);
            }
            etapes.push(etape);
        }
        let journaux = chemins.racine_guioxide().join("journaux");
        let chaine = Chaine::lancer(ctx, etapes, journaux)?;
        self.chaine = Some((chaine, seance.clone()));
        self.enregistrees.clear();
        self.suivi = true;
        Ok(())
    }

    /// Fenêtre de suivi : état de chaque étape, avancement estimé, fin du journal de l'étape en cours, annulation.
    fn fenetre_suivi(&mut self, ctx: &egui::Context) {
        let Some((chaine, seance)) = &self.chaine else { return };
        if !chaine.finie() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        let mut ouverte = self.suivi;
        egui::Window::new(format!("Suivi · {}", seance.libelle))
            .id(egui::Id::unique("fenetre_suivi"))
            .open(&mut ouverte)
            .default_width(640.0)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -40.0])
            .show(ctx, |ui| {
                let etats = chaine.etats();
                egui::Grid::new("etapes_chaine")
                    .num_columns(3)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        for (i, (etape, etat)) in chaine.etapes.iter().zip(&etats).enumerate() {
                            let (signe, couleur) = match etat {
                                EtatEtape::Attente => ("·", theme::TEXTE_SECONDAIRE),
                                EtatEtape::EnCours => ("▶", theme::ACCENT),
                                EtatEtape::Terminee(_) => ("✓", theme::TERMINE),
                                EtatEtape::Echec(_) => ("✗", theme::ERREUR),
                                EtatEtape::Annulee => ("–", theme::TEXTE_SECONDAIRE),
                            };
                            ui.label(RichText::new(signe).monospace().color(couleur));
                            ui.label(RichText::new(&etape.nom).monospace())
                                .on_hover_text(etape.ligne_de_commande());
                            ui.horizontal(|ui| {
                                match etat {
                                    EtatEtape::EnCours => avancement(ui, chaine, i),
                                    EtatEtape::Terminee(d) => {
                                        ui.label(RichText::new(duree_lisible(*d)).color(couleur));
                                    }
                                    EtatEtape::Echec(e) => {
                                        ui.label(RichText::new(e).color(couleur));
                                    }
                                    EtatEtape::Attente => {
                                        let t = etape
                                            .prevue
                                            .map_or(String::new(), |d| format!("≈ {}", duree_lisible(d)));
                                        ui.label(RichText::new(t).color(theme::TEXTE_SECONDAIRE));
                                    }
                                    EtatEtape::Annulee => {
                                        ui.label(RichText::new("non lancée").color(couleur));
                                    }
                                }
                                if !matches!(etat, EtatEtape::Attente | EtatEtape::Annulee)
                                    && ui.small_button("journal").clicked()
                                {
                                    let _ = std::process::Command::new("open").arg(chaine.journal(i)).spawn();
                                }
                            });
                            ui.end_row();
                        }
                    });
                if let Some(i) = chaine.courante() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(Color32::from_black_alpha(40))
                        .corner_radius(6.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let fin = chaine::fin_du_journal(&chaine.journal(i), 8);
                            let texte = if fin.lines().count() <= 2 {
                                format!("{fin}\n(l'outil n'écrit rien pendant le calcul : avancement estimé)")
                            } else {
                                fin
                            };
                            ui.label(RichText::new(texte).small().monospace());
                        });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if chaine.finie() {
                        ui.label(format!("Fini en {}", duree_lisible(chaine.debut.elapsed())));
                    } else {
                        if ui.button("Annuler").clicked() {
                            chaine.annuler();
                        }
                        ui.label(RichText::new(duree_lisible(chaine.debut.elapsed())).color(theme::TEXTE_SECONDAIRE));
                    }
                    if ui.button("Dossier des journaux").clicked() {
                        let _ = std::process::Command::new("open").arg(&chaine.journaux).spawn();
                    }
                });
            });
        self.suivi = ouverte;
    }
}

/// Barre d'avancement de l'étape en cours : pas lus dans le journal si l'outil les écrit, sinon temps écoulé
/// rapporté à la durée prévue.
fn avancement(ui: &mut Ui, chaine: &Chaine, i: usize) {
    let etape = &chaine.etapes[i];
    let ecoule = chaine.duree_etape_courante();
    let pas = etape
        .pas
        .and_then(|total| Some((chaine::pas_atteint(&chaine.journal(i))?, total)));
    let (fraction, texte) = match (pas, etape.prevue) {
        (Some((n, total)), _) => {
            let f = n as f32 / total.max(1) as f32;
            let reste = if n > 0 {
                format!(" · reste ≈ {}", duree_lisible(ecoule.mul_f32((1.0 - f) / f.max(1e-6))))
            } else {
                String::new()
            };
            (f, format!("pas {n} / {total}{reste}"))
        }
        (None, Some(prevue)) => {
            let f = (ecoule.as_secs_f32() / prevue.as_secs_f32().max(1.0)).min(0.99);
            let reste = prevue.saturating_sub(ecoule);
            let texte = if ecoule > prevue {
                format!(
                    "{} · plus long que prévu ({})",
                    duree_lisible(ecoule),
                    duree_lisible(prevue)
                )
            } else {
                format!("{} · reste ≈ {}", duree_lisible(ecoule), duree_lisible(reste))
            };
            (f, texte)
        }
        (None, None) => (0.0, format!("{} · pas d'estimation", duree_lisible(ecoule))),
    };
    ui.add(
        egui::ProgressBar::new(fraction)
            .desired_width(180.0)
            .fill(theme::ACCENT)
            .animate(pas.is_none() && etape.prevue.is_none()),
    );
    ui.label(RichText::new(texte).small().color(theme::TEXTE_SECONDAIRE));
}

/// Clé des durées et quantité de travail d'une étape : `piles` compte toutes les piles T2w de la séance (fetalbet
/// les traite toutes), `coupes` celles des piles utilisées par le SVR.
fn travail(c: Calcul, r: &Reglages, piles: usize, coupes: usize) -> (String, f64) {
    match c {
        Calcul::Masques => ("masques".into(), piles as f64),
        Calcul::Controle => ("controle".into(), coupes as f64),
        Calcul::Reconstruction => {
            let mode = match r.mouvement {
                Mouvement::S2s => "s2s",
                Mouvement::S2v => "s2v",
            };
            let materiel = if r.gpu { "-gpu" } else { "" };
            // Le coût croît avec le nombre de voxels du volume : (0,8 / résolution)³ à nombre de coupes égal.
            let voxels = (0.8 / r.resolution.max(0.1)).powi(3);
            (format!("svr-{mode}{materiel}"), coupes as f64 * voxels)
        }
        Calcul::Segmentation => ("segmentation".into(), 1.0),
        Calcul::Surfaces => ("surfaces".into(), 1.0),
        Calcul::Maillage => ("maillage".into(), 1.0),
        Calcul::Simulation => ("simulation".into(), r.pas as f64),
    }
}

/// Durée prévue : mesure de cette machine, sinon mesure de référence.
fn self_estimation(durees: &Durees, cle: &str, unites: f64) -> Option<Duration> {
    let par_unite = durees.get(cle).copied().or_else(|| duree_de_reference(cle))?;
    Some(Duration::from_secs_f64(par_unite * unites))
}

/// `45 s`, `3 min`, `1 h 05`.
pub fn duree_lisible(d: Duration) -> String {
    let s = d.as_secs();
    if s < 90 {
        format!("{s} s")
    } else if s < 3600 {
        format!("{} min", (s + 30) / 60)
    } else {
        format!("{} h {:02}", s / 3600, (s % 3600) / 60)
    }
}

/// Vrai si les sorties de l'étape existent déjà.
fn deja_fait(c: Calcul, chemins: &CheminsSession, session: &Session, hemi: Hemisphere) -> bool {
    let h = match hemi {
        Hemisphere::Gauche => "L",
        Hemisphere::Droit => "R",
    };
    match c {
        Calcul::Masques => {
            let t2w: Vec<_> = session.piles.iter().filter(|p| p.contraste == "T2w").collect();
            !t2w.is_empty() && t2w.iter().all(|p| chemins.masque(p).exists())
        }
        Calcul::Controle => chemins.qualite_acquisition().exists(),
        Calcul::Reconstruction => chemins.svr().exists(),
        Calcul::Segmentation => chemins.segmentation().exists(),
        Calcul::Surfaces => chemins.surface("pial", Some(h)).exists(),
        Calcul::Maillage => chemins
            .anat("gyroxide")
            .join(format!("{}_hemi-{h}_desc-tetgen_tet.vtu", chemins.prefixe()))
            .exists(),
        Calcul::Simulation => chemins.simulations().join("run-guioxide").join("status.json").exists(),
    }
}

/// Séance de la session choisie, telle que les outils la désignent.
pub fn seance(jeu: &guioxide_bids::JeuBids, sujet: usize, session: usize, sortie: Option<&Path>) -> Seance {
    let s = &jeu.sujets[sujet];
    let ses = &s.sessions[session];
    let participant = s.id.strip_prefix("sub-").unwrap_or(&s.id).to_owned();
    let id_session = ses.id.as_ref().map(|i| i.strip_prefix("ses-").unwrap_or(i).to_owned());
    let libelle = match &ses.id {
        Some(i) => format!("{} / {i}", s.id),
        None => s.id.clone(),
    };
    Seance {
        racine: jeu.racine.clone(),
        participant,
        session: id_session,
        sortie: sortie.map(Path::to_path_buf),
        libelle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durees_lisibles() {
        assert_eq!(duree_lisible(Duration::from_secs(42)), "42 s");
        assert_eq!(duree_lisible(Duration::from_secs(781)), "13 min");
        assert_eq!(duree_lisible(Duration::from_secs(3900)), "1 h 05");
    }

    #[test]
    fn estimation_par_unite_et_resolution() {
        let r = Reglages::default();
        let (cle, unites) = travail(Calcul::Reconstruction, &r, 5, 95);
        assert_eq!(cle, "svr-s2s");
        assert!((unites - 95.0).abs() < 1e-9);
        let fine = Reglages {
            resolution: 0.4,
            ..Reglages::default()
        };
        assert!((travail(Calcul::Reconstruction, &fine, 5, 95).1 - 95.0 * 8.0).abs() < 1e-6);
        let mut durees = Durees::new();
        assert_eq!(
            self_estimation(&durees, "svr-s2s", 100.0),
            Some(Duration::from_secs_f64(820.0))
        );
        durees.insert("svr-s2s".into(), 2.0);
        assert_eq!(
            self_estimation(&durees, "svr-s2s", 100.0),
            Some(Duration::from_secs(200))
        );
        assert_eq!(self_estimation(&durees, "svr-s2v", 100.0), None);
    }

    #[test]
    fn arguments_de_la_seance() {
        let s = Seance {
            racine: PathBuf::from("/e"),
            participant: "str002".into(),
            session: Some("01".into()),
            sortie: Some(PathBuf::from("/t")),
            libelle: String::new(),
        };
        assert_eq!(
            s.arguments(&["svr", "qc"]),
            [
                "svr",
                "qc",
                "--bids",
                "/e",
                "--participant",
                "str002",
                "--session",
                "01",
                "--output-dir",
                "/t"
            ]
        );
    }

    #[test]
    fn prealables_dans_l_ordre() {
        for c in Calcul::TOUS {
            if let Some(p) = c.prealable() {
                assert!(p < c);
            }
        }
    }

    /// Étapes réelles par le même chemin que le bouton : `GUIOXIDE_ETUDE` (jeu BIDS d'une séance, modifié) ;
    /// masques puis contrôle des piles, la dernière pile écartée.
    #[test]
    #[ignore = "outils de la suite et piles réelles"]
    fn etapes_reelles() {
        let racine = PathBuf::from(std::env::var("GUIOXIDE_ETUDE").expect("GUIOXIDE_ETUDE"));
        let jeu = guioxide_bids::indexer(&racine).unwrap();
        let seance = seance(&jeu, 0, 0, None);
        let (sujet, session) = (&jeu.sujets[0], &jeu.sujets[0].sessions[0]);
        let chemins = CheminsSession::new(&jeu.derivees_etude(), sujet, session);
        let piles: Vec<PathBuf> = session.piles.iter().map(|p| p.chemin.clone()).collect();
        let utilisees = &piles[..piles.len() - 1];
        let ctx = egui::Context::default();
        let mut calculs = Calculs::default();
        calculs
            .lancer(
                &ctx,
                &seance,
                &chemins,
                &[Calcul::Masques, Calcul::Controle],
                utilisees,
                piles.len(),
                76,
            )
            .unwrap();
        let (chaine, _) = calculs.chaine.as_ref().unwrap();
        assert!(chaine.etapes[1].arguments.contains(&"--input".to_owned()));
        while !chaine.finie() {
            std::thread::sleep(Duration::from_secs(2));
        }
        let etats = chaine.etats();
        for (i, e) in etats.iter().enumerate() {
            eprintln!(
                "{} : {e:?}\n{}",
                chaine.etapes[i].nom,
                chaine::fin_du_journal(&chaine.journal(i), 6)
            );
        }
        assert!(etats.iter().all(|e| matches!(e, EtatEtape::Terminee(_))));
        assert!(deja_fait(Calcul::Masques, &chemins, session, Hemisphere::Gauche));
        assert!(deja_fait(Calcul::Controle, &chemins, session, Hemisphere::Gauche));
        calculs.memoriser_durees();
        eprintln!("durées mémorisées : {:?}", calculs.durees);
        assert!(calculs.durees.contains_key("masques") && calculs.durees.contains_key("controle"));
    }
}
