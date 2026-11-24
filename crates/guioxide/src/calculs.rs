//! Calculs d'une séance, montrés dans la barre d'étapes : chaque calcul se lance seul, jusqu'à lui (ce qui manque
//! avant), ou toute la suite ; réglages de la reconstruction (résolution, mouvement, piles utilisées…) ; avancement et
//! temps restant estimés d'après les exécutions précédentes sur cette machine ; journal en tiroir ; arrêt.

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

    /// Nom court, dans la barre d'étapes.
    pub fn court(self) -> &'static str {
        match self {
            Calcul::Masques => "masques",
            Calcul::Controle => "contrôle",
            Calcul::Reconstruction => "SVR",
            Calcul::Segmentation => "segmentation",
            Calcul::Surfaces => "surfaces",
            Calcul::Maillage => "maillage",
            Calcul::Simulation => "simulation",
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

/// Ce dont les commandes ont besoin : la séance choisie, telle que les outils et le contrat la désignent.
pub struct Contexte<'a> {
    pub seance: &'a Seance,
    pub session: &'a Session,
    pub chemins: &'a CheminsSession,
}

/// Calculs montrés sous chaque étape de la barre d'étapes.
pub fn calculs_de(etape: guioxide_bids::Etape) -> &'static [Calcul] {
    use guioxide_bids::Etape as E;
    match etape {
        E::Examen => &[Calcul::Masques, Calcul::Controle],
        E::Reconstruction => &[Calcul::Reconstruction, Calcul::Segmentation],
        E::Surfaces => &[Calcul::Surfaces],
        E::Simulation => &[Calcul::Maillage, Calcul::Simulation],
    }
}

/// Chaîne lancée : processus, séance, calcul de chaque étape.
pub struct ChaineLancee {
    pub chaine: Chaine,
    pub seance: Seance,
    calculs: Vec<Calcul>,
}

/// Piles T2w d'une séance : nombre total, chemins de celles gardées pour la reconstruction, leurs coupes.
struct Piles {
    total: usize,
    utilisees: Vec<PathBuf>,
    coupes: usize,
}

/// État d'un calcul dans la barre d'étapes.
enum EtatCalcul {
    Fait,
    AFaire,
    Attente,
    EnCours { fraction: f32, texte: String },
    Echec(String),
}

/// Calculs de la séance : réglages, chaîne en cours, journal.
#[derive(Default)]
pub struct Calculs {
    pub reglages: Reglages,
    pub durees: Durees,
    /// Piles retirées de la reconstruction, par séance (nom de la pile).
    exclues: BTreeMap<String, BTreeSet<String>>,
    pub chaine: Option<ChaineLancee>,
    /// Étapes dont la durée a déjà été mémorisée, pour la chaîne en cours.
    enregistrees: BTreeSet<usize>,
    erreur: Option<String>,
    /// Nombre de coupes de chaque pile, lu une fois dans l'en-tête.
    coupes: BTreeMap<PathBuf, usize>,
    /// Tiroir du journal ouvert au bas de la fenêtre.
    pub journal: bool,
    /// Étape dont le tiroir montre le journal ; `None` : l'étape en cours (ou la dernière).
    journal_etape: Option<usize>,
}

fn cle_seance(seance: &Seance) -> String {
    format!(
        "{}|{}|{:?}",
        seance.racine.display(),
        seance.participant,
        seance.session
    )
}

impl Calculs {
    pub fn en_cours(&self) -> bool {
        self.chaine.as_ref().is_some_and(|c| !c.chaine.finie())
    }

    /// Vrai si une étape s'est terminée depuis le dernier appel : les dérivées sont à relire.
    pub fn a_relire(&self) -> bool {
        self.chaine.as_ref().is_some_and(|c| c.chaine.a_relire())
    }

    /// À chaque image : mémorise la durée des étapes terminées, redessine tant qu'un calcul tourne.
    pub fn actualiser(&mut self, ctx: &egui::Context) {
        self.memoriser_durees();
        if self.en_cours() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn memoriser_durees(&mut self) {
        let Some(c) = &self.chaine else { return };
        for (i, etat) in c.chaine.etats().iter().enumerate() {
            if let EtatEtape::Terminee(d) = etat
                && self.enregistrees.insert(i)
            {
                let e = &c.chaine.etapes[i];
                self.durees.insert(e.cle.clone(), d.as_secs_f64() / e.unites.max(1e-9));
            }
        }
    }

    fn piles(&mut self, cx: &Contexte) -> Piles {
        let exclues = self.exclues.get(&cle_seance(cx.seance));
        let t2w: Vec<&guioxide_bids::Pile> = cx.session.piles.iter().filter(|p| p.contraste == "T2w").collect();
        let utilisees: Vec<PathBuf> = t2w
            .iter()
            .filter(|p| exclues.is_none_or(|e| !e.contains(&p.nom)))
            .map(|p| p.chemin.clone())
            .collect();
        let cache = &mut self.coupes;
        let coupes = utilisees
            .iter()
            .map(|p| {
                *cache
                    .entry(p.clone())
                    .or_insert_with(|| guioxide_data::lire_geometrie(p).map_or(0, |(d, _)| d[2]))
            })
            .sum();
        Piles {
            total: t2w.len(),
            utilisees,
            coupes,
        }
    }

    /// Durée estimée d'un calcul pour la séance.
    fn estimation(&self, c: Calcul, piles: &Piles) -> Option<Duration> {
        let (cle, unites) = travail(c, &self.reglages, piles.total, piles.coupes);
        self_estimation(&self.durees, &cle, unites)
    }

    /// État d'un calcul : celui de la chaîne lancée sur cette séance s'il en fait partie (et n'y a pas réussi), sinon
    /// la présence de ses sorties.
    fn etat(&self, c: Calcul, cx: &Contexte) -> EtatCalcul {
        if let Some(l) = self.chaine.as_ref().filter(|l| l.seance == *cx.seance)
            && let Some(i) = l.calculs.iter().rposition(|x| *x == c)
        {
            match &l.chaine.etats()[i] {
                EtatEtape::Attente => return EtatCalcul::Attente,
                EtatEtape::EnCours => {
                    let (fraction, texte, _) = avancement(&l.chaine, i, true);
                    return EtatCalcul::EnCours { fraction, texte };
                }
                EtatEtape::Echec(e) => return EtatCalcul::Echec(e.clone()),
                EtatEtape::Terminee(_) | EtatEtape::Annulee => {}
            }
        }
        if deja_fait(c, cx.chemins, cx.session, self.reglages.hemisphere) {
            EtatCalcul::Fait
        } else {
            EtatCalcul::AFaire
        }
    }

    /// Calculs qui manquent, dans l'ordre, jusqu'à `fin` comprise (tous si `None`) ; `fin` y est toujours.
    fn manquants(&self, cx: &Contexte, fin: Option<Calcul>) -> Vec<Calcul> {
        Calcul::TOUS
            .into_iter()
            .filter(|c| fin.is_none_or(|f| *c <= f))
            .filter(|c| Some(*c) == fin || !deja_fait(*c, cx.chemins, cx.session, self.reglages.hemisphere))
            .collect()
    }

    fn lancer_pour(&mut self, ctx: &egui::Context, cx: &Contexte, calculs: &[Calcul]) {
        let piles = self.piles(cx);
        let resultat = self.lancer(
            ctx,
            cx.seance,
            cx.chemins,
            calculs,
            &piles.utilisees,
            piles.total,
            piles.coupes,
        );
        self.erreur = resultat.err();
    }

    /// Pastille d'un calcul dans la barre d'étapes : état, avancement ; un clic ouvre le menu (lancer, lancer jusqu'ici,
    /// journal).
    pub fn pastille(&mut self, ui: &mut Ui, c: Calcul, cx: &Contexte) {
        let etat = self.etat(c, cx);
        let (signe, couleur) = match &etat {
            EtatCalcul::Fait => ("✓", theme::TERMINE),
            EtatCalcul::AFaire => ("○", theme::TEXTE_SECONDAIRE),
            EtatCalcul::Attente => ("·", theme::TEXTE_SECONDAIRE),
            EtatCalcul::EnCours { .. } => ("▶", theme::ACCENT),
            EtatCalcul::Echec(_) => ("✗", theme::ERREUR),
        };
        let mut texte = format!("{signe} {}", c.court());
        if let EtatCalcul::EnCours { texte: t, .. } = &etat {
            texte = format!("{texte} · {t}");
        }
        let bouton = egui::Button::new(RichText::new(texte).size(12.5).color(couleur))
            .small()
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, theme::BORDURE))
            .corner_radius(10);
        let reponse = ui.add(bouton);
        if let EtatCalcul::EnCours { fraction, .. } = &etat {
            let r = reponse.rect.shrink2(egui::vec2(8.0, 0.0));
            let y = r.bottom() - 2.0;
            let peintre = ui.painter();
            peintre.line_segment(
                [egui::pos2(r.left(), y), egui::pos2(r.right(), y)],
                egui::Stroke::new(2.0, theme::BORDURE),
            );
            peintre.line_segment(
                [egui::pos2(r.left(), y), egui::pos2(r.left() + r.width() * fraction, y)],
                egui::Stroke::new(2.0, theme::ACCENT),
            );
        }
        let piles = self.piles(cx);
        let occupe = self.en_cours();
        let mut lancer: Option<Vec<Calcul>> = None;
        egui::Popup::menu(&reponse)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(320.0)
            .show(|ui| {
                ui.label(RichText::new(c.libelle()).strong());
                ui.label(
                    RichText::new(c.commande())
                        .monospace()
                        .small()
                        .color(theme::TEXTE_SECONDAIRE),
                );
                let etat_texte = match &etat {
                    EtatCalcul::Fait => "sorties présentes".to_owned(),
                    EtatCalcul::AFaire => "pas encore calculé".to_owned(),
                    EtatCalcul::Attente => "en attente dans la chaîne".to_owned(),
                    EtatCalcul::EnCours { texte, .. } => format!("en cours : {texte}"),
                    EtatCalcul::Echec(e) => format!("échec : {e}"),
                };
                ui.label(RichText::new(etat_texte).color(couleur));
                if let Some(d) = self.estimation(c, &piles) {
                    ui.label(
                        RichText::new(format!("durée estimée ≈ {}", duree_lisible(d))).color(theme::TEXTE_SECONDAIRE),
                    );
                }
                ui.separator();
                let verbe = if matches!(etat, EtatCalcul::Fait) {
                    "Relancer"
                } else {
                    "Lancer"
                };
                let manque = c
                    .prealable()
                    .filter(|p| !deja_fait(*p, cx.chemins, cx.session, self.reglages.hemisphere));
                if let Some(p) = manque {
                    ui.label(
                        RichText::new(format!("Il manque d'abord « {} ».", p.libelle().to_lowercase()))
                            .color(theme::ACCENT),
                    );
                }
                let seul = ui.add_enabled(
                    !occupe && manque.is_none(),
                    egui::Button::new(format!("{verbe} {}", c.court())),
                );
                if seul.clicked() {
                    lancer = Some(vec![c]);
                }
                let jusqu_ici = self.manquants(cx, Some(c));
                if jusqu_ici.len() > 1 {
                    let duree: Option<Duration> = jusqu_ici.iter().map(|x| self.estimation(*x, &piles)).sum();
                    let texte = format!(
                        "Lancer jusqu'ici ({} étapes{})",
                        jusqu_ici.len(),
                        duree.map_or(String::new(), |d| format!(", ≈ {}", duree_lisible(d)))
                    );
                    let noms: Vec<&str> = jusqu_ici.iter().map(|x| x.court()).collect();
                    if ui
                        .add_enabled(!occupe, egui::Button::new(texte))
                        .on_hover_text(noms.join(" → "))
                        .clicked()
                    {
                        lancer = Some(jusqu_ici);
                    }
                }
                if occupe {
                    ui.label(
                        RichText::new("un calcul est en cours")
                            .small()
                            .color(theme::TEXTE_SECONDAIRE),
                    );
                }
                if let Some(l) = self.chaine.as_ref().filter(|l| l.seance == *cx.seance)
                    && let Some(i) = l.calculs.iter().rposition(|x| *x == c)
                    && !matches!(l.chaine.etats()[i], EtatEtape::Attente | EtatEtape::Annulee)
                    && ui.button("Voir le journal").clicked()
                {
                    self.journal = true;
                    self.journal_etape = Some(i);
                }
            });
        if let Some(calculs) = lancer {
            egui::Popup::close_all(ui.ctx());
            self.lancer_pour(ui.ctx(), cx, &calculs);
        }
    }

    /// Boutons de droite de la barre d'étapes : réglages, lancer la suite ou arrêter.
    pub fn commandes(&mut self, ui: &mut Ui, cx: &Contexte) {
        if let Some(l) = self.chaine.as_ref().filter(|l| !l.chaine.finie()) {
            let arreter = egui::Button::new(RichText::new("Arrêter").strong()).fill(theme::ERREUR.gamma_multiply(0.25));
            if ui
                .add(arreter)
                .on_hover_text(format!("arrêter les calculs de {}", l.seance.libelle))
                .clicked()
            {
                l.chaine.annuler();
            }
        } else {
            let suite = self.manquants(cx, None);
            let piles = self.piles(cx);
            let duree: Option<Duration> = suite.iter().map(|c| self.estimation(*c, &piles)).sum();
            let texte = match (suite.len(), duree) {
                (0, _) => "Tout est calculé".to_owned(),
                (n, Some(d)) => format!("Lancer la suite · {n} · ≈ {}", duree_lisible(d)),
                (n, None) => format!("Lancer la suite · {n}"),
            };
            let bouton = egui::Button::new(RichText::new(texte).strong()).fill(theme::ACCENT_DOUX);
            let noms: Vec<&str> = suite.iter().map(|c| c.court()).collect();
            if ui
                .add_enabled(!suite.is_empty(), bouton)
                .on_hover_text(format!("calculs qui manquent, dans l'ordre : {}", noms.join(" → ")))
                .clicked()
            {
                self.lancer_pour(ui.ctx(), cx, &suite);
            }
        }
        let reglages = ui.button("Réglages");
        egui::Popup::menu(&reglages)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(440.0)
            .show(|ui| self.reglages_ui(ui, cx));
    }

    /// Réglages des calculs : reconstruction, piles utilisées, simulation.
    fn reglages_ui(&mut self, ui: &mut Ui, cx: &Contexte) {
        let piles: Vec<&guioxide_bids::Pile> = cx.session.piles.iter().filter(|p| p.contraste == "T2w").collect();
        let comptes = self.piles(cx);
        let exclues = self.exclues.entry(cle_seance(cx.seance)).or_default();
        let r = &mut self.reglages;
        theme::titre_section(ui, "Reconstruction");
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
                    ui.radio_value(&mut r.mouvement, Mouvement::S2s, "coupe-coupe");
                    ui.radio_value(&mut r.mouvement, Mouvement::S2v, "coupe-volume (plus long)");
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
                    ui.radio_value(&mut r.gpu, true, "GPU (Metal)")
                        .on_hover_text("medx : le chemin GPU de la SVR est aujourd'hui souvent plus lent que le CPU");
                });
                ui.end_row();
            });
        theme::titre_section(
            ui,
            &format!(
                "Piles utilisées · {} sur {} · {} coupes",
                comptes.utilisees.len(),
                comptes.total,
                comptes.coupes
            ),
        );
        for p in &piles {
            let mut garder = !exclues.contains(&p.nom);
            let nom = crate::etapes::nom_court(&p.nom, cx.chemins.prefixe()).to_owned();
            if ui.checkbox(&mut garder, RichText::new(nom).monospace()).changed() {
                if garder {
                    exclues.remove(&p.nom);
                } else {
                    exclues.insert(p.nom.clone());
                }
            }
        }
        if comptes.utilisees.len() < 2 {
            ui.label(RichText::new("deux piles au moins").color(theme::ERREUR));
        }
        theme::titre_section(ui, "Simulation");
        ui.horizontal(|ui| {
            ui.label("Hémisphère");
            ui.radio_value(&mut r.hemisphere, Hemisphere::Gauche, "gauche");
            ui.radio_value(&mut r.hemisphere, Hemisphere::Droit, "droit");
            ui.separator();
            ui.add(egui::DragValue::new(&mut r.pas).range(10..=100_000).suffix(" pas"));
        });
    }

    /// Barre de pied : calcul en cours et temps restant, ou bilan de la dernière chaîne ; bouton du journal.
    pub fn pied(&mut self, ui: &mut Ui) {
        if let Some(e) = &self.erreur {
            ui.separator();
            ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
        }
        let Some(l) = &self.chaine else { return };
        ui.separator();
        let etats = l.chaine.etats();
        if let Some(i) = l.chaine.courante() {
            let (fraction, texte, indefini) = avancement(&l.chaine, i, false);
            ui.label(RichText::new(format!("▶ {}", l.chaine.etapes[i].nom)).color(theme::ACCENT));
            ui.add(
                egui::ProgressBar::new(fraction)
                    .desired_width(140.0)
                    .desired_height(6.0)
                    .fill(theme::ACCENT)
                    .animate(indefini),
            );
            ui.label(RichText::new(texte).color(theme::TEXTE_SECONDAIRE));
        } else if let Some((i, EtatEtape::Echec(e))) =
            etats.iter().enumerate().find(|(_, e)| matches!(e, EtatEtape::Echec(_)))
        {
            ui.label(RichText::new(format!("✗ {} : {e}", l.chaine.etapes[i].nom)).color(theme::ERREUR));
        } else if etats.iter().any(|e| matches!(e, EtatEtape::Annulee)) {
            ui.label(RichText::new("calculs arrêtés").color(theme::TEXTE_SECONDAIRE));
        } else {
            ui.label(
                RichText::new(format!(
                    "✓ calculs finis en {} · {}",
                    duree_lisible(l.chaine.debut.elapsed()),
                    l.seance.libelle
                ))
                .color(theme::TERMINE),
            );
        }
        let texte = if self.journal { "Journal ▾" } else { "Journal ▴" };
        if ui.small_button(texte).on_hover_text("sortie des outils").clicked() {
            self.journal = !self.journal;
        }
    }

    /// Tiroir du journal : étapes de la chaîne (un clic montre son journal), fin du journal choisi.
    pub fn tiroir(&mut self, ui: &mut Ui) {
        let Some(l) = &self.chaine else {
            ui.label(RichText::new("aucun calcul lancé").color(theme::TEXTE_SECONDAIRE));
            return;
        };
        let etats = l.chaine.etats();
        let courante = l.chaine.courante().unwrap_or(etats.len().saturating_sub(1));
        let montree = self.journal_etape.filter(|i| *i < etats.len()).unwrap_or(courante);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&l.seance.libelle).strong());
            for (i, (etape, etat)) in l.chaine.etapes.iter().zip(&etats).enumerate() {
                let (signe, couleur) = match etat {
                    EtatEtape::Attente => ("·", theme::TEXTE_SECONDAIRE),
                    EtatEtape::EnCours => ("▶", theme::ACCENT),
                    EtatEtape::Terminee(_) => ("✓", theme::TERMINE),
                    EtatEtape::Echec(_) => ("✗", theme::ERREUR),
                    EtatEtape::Annulee => ("–", theme::TEXTE_SECONDAIRE),
                };
                let duree = match etat {
                    EtatEtape::Terminee(d) => format!(" {}", duree_lisible(*d)),
                    _ => String::new(),
                };
                let texte = RichText::new(format!("{signe} {}{duree}", etape.nom)).color(couleur);
                if ui
                    .selectable_label(i == montree, texte)
                    .on_hover_text(etape.ligne_de_commande())
                    .clicked()
                {
                    self.journal_etape = Some(i);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Fermer").clicked() {
                    self.journal = false;
                }
                if ui.small_button("Dossier des journaux").clicked() {
                    let _ = std::process::Command::new("open").arg(&l.chaine.journaux).spawn();
                }
            });
        });
        ui.separator();
        let fin = chaine::fin_du_journal(&l.chaine.journal(montree), 400);
        let fin = if matches!(etats[montree], EtatEtape::EnCours) && fin.lines().count() <= 2 {
            format!("{fin}\n(l'outil n'écrit rien pendant le calcul : avancement estimé d'après les durées passées)")
        } else {
            fin
        };
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(RichText::new(fin).monospace().size(11.5));
            });
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
        self.chaine = Some(ChaineLancee {
            chaine,
            seance: seance.clone(),
            calculs: calculs.to_vec(),
        });
        self.enregistrees.clear();
        self.journal_etape = None;
        Ok(())
    }
}

/// Avancement de l'étape `i` en cours : fraction, texte (court pour la barre d'étapes), et vrai si l'avancement est
/// inconnu. Pas lus dans le journal si l'outil les écrit, sinon temps écoulé rapporté à la durée prévue.
fn avancement(chaine: &Chaine, i: usize, court: bool) -> (f32, String, bool) {
    let etape = &chaine.etapes[i];
    let ecoule = chaine.duree_etape_courante();
    let pas = etape
        .pas
        .and_then(|total| Some((chaine::pas_atteint(&chaine.journal(i))?, total)));
    match (pas, etape.prevue) {
        (Some((n, total)), _) => {
            let f = n as f32 / total.max(1) as f32;
            let reste = (n > 0).then(|| duree_lisible(ecoule.mul_f32((1.0 - f) / f.max(1e-6))));
            let texte = match (court, reste) {
                (true, Some(r)) => format!("{} % · {r}", (f * 100.0) as u32),
                (true, None) => format!("{} %", (f * 100.0) as u32),
                (false, Some(r)) => format!("pas {n} / {total} · reste ≈ {r}"),
                (false, None) => format!("pas {n} / {total}"),
            };
            (f, texte, false)
        }
        (None, Some(prevue)) => {
            let f = (ecoule.as_secs_f32() / prevue.as_secs_f32().max(1.0)).min(0.99);
            let reste = prevue.saturating_sub(ecoule);
            let texte = match (court, ecoule > prevue) {
                (true, false) => format!("{} % · {}", (f * 100.0) as u32, duree_lisible(reste)),
                (true, true) => format!("{} (prévu {})", duree_lisible(ecoule), duree_lisible(prevue)),
                (false, false) => format!("{} · reste ≈ {}", duree_lisible(ecoule), duree_lisible(reste)),
                (false, true) => format!(
                    "{} · plus long que prévu ({})",
                    duree_lisible(ecoule),
                    duree_lisible(prevue)
                ),
            };
            (f, texte, false)
        }
        (None, None) => (0.0, duree_lisible(ecoule), true),
    }
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
        let chaine = &calculs.chaine.as_ref().unwrap().chaine;
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
