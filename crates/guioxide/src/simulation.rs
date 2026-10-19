//! Onglet 4 · Simulation : runs de gyroxide, trois vues 3D synchronisées (état initial, pas courant, surface
//! observée d'une session ultérieure), frise temporelle et lecture, courbes, relecture d'un run en cours.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use egui::{RichText, Ui, Vec2};
use egui_plot::{Line, Plot, PlotPoints, VLine};
use egui_wgpu::RenderState;
use guioxide_bids::{CheminsSession, JeuBids};
use guioxide_data::{EtatRun, Maillage, PasSequence, lire_etat_run, lire_sequence, lister_runs};
use guioxide_geom::PointMonde;
use guioxide_view::theme;
use guioxide_view::vue3d::{Camera, Objet3d, Vue3d};

use crate::etapes::{Base, Fichier, Statut, icone_statut};

/// Intervalle de relecture d'un run en cours.
const RELECTURE: Duration = Duration::from_secs(2);
/// Durée d'affichage d'un pas pendant la lecture.
const CADENCE: Duration = Duration::from_millis(400);
/// Étendue (mm) sous laquelle une surface est jugée en coordonnées normalisées.
const ETENDUE_MINIMALE_MM: f32 = 10.0;

const COULEUR_SIMULEE: [u8; 3] = [0xF2, 0xD0, 0x4B];
const COULEUR_OBSERVEE: [u8; 3] = [0x7F, 0xB3, 0xE6];

/// Un run et ses fichiers, relus quand ils changent.
struct Run {
    label: String,
    dossier: PathBuf,
    etat: Fichier<EtatRun>,
    sequence: Fichier<Vec<PasSequence>>,
    config: Option<String>,
    /// Dates de modification de `status.json` et `sequence.tsv` à la dernière lecture.
    versions: (Option<SystemTime>, Option<SystemTime>),
}

fn version(chemin: &Path) -> Option<SystemTime> {
    std::fs::metadata(chemin).and_then(|m| m.modified()).ok()
}

fn lire_fichier<T>(chemin: &Path, lire: impl FnOnce(&Path) -> Result<T, guioxide_data::ErreurDonnees>) -> Fichier<T> {
    if !chemin.exists() {
        return Fichier::Absent;
    }
    match lire(chemin) {
        Ok(v) => Fichier::Lu(v),
        Err(e) => Fichier::Erreur(e.to_string()),
    }
}

impl Run {
    fn lire(label: String, dossier: PathBuf) -> Run {
        let mut run = Run {
            label,
            config: std::fs::read_to_string(dossier.join("config.toml")).ok(),
            dossier,
            etat: Fichier::Absent,
            sequence: Fichier::Absent,
            versions: (None, None),
        };
        run.relire();
        run
    }

    /// Relit `status.json` et `sequence.tsv` s'ils ont changé.
    fn relire(&mut self) {
        let (statut, sequence) = (self.dossier.join("status.json"), self.dossier.join("sequence.tsv"));
        let versions = (version(&statut), version(&sequence));
        if versions.0 != self.versions.0 || matches!(self.etat, Fichier::Absent) {
            self.etat = lire_fichier(&statut, lire_etat_run);
        }
        if versions.1 != self.versions.1 || matches!(self.sequence, Fichier::Absent) {
            self.sequence = lire_fichier(&sequence, lire_sequence);
        }
        self.versions = versions;
    }

    fn en_cours(&self) -> bool {
        self.etat.lu().is_some_and(|e| e.state == "running")
    }

    fn pas(&self) -> &[PasSequence] {
        self.sequence.lu().map(Vec::as_slice).unwrap_or_default()
    }

    /// Hémisphère des surfaces simulées, d'après le nom des fichiers (`hemi-L`, `hemi-R`).
    fn hemisphere(&self) -> Option<&'static str> {
        let f = &self.pas().first()?.file;
        if f.contains("hemi-L") {
            Some("L")
        } else if f.contains("hemi-R") {
            Some("R")
        } else {
            None
        }
    }
}

pub struct Simulation {
    runs: Option<Vec<Run>>,
    choisi: usize,
    index: usize,
    lecture: bool,
    dernier_pas: Instant,
    derniere_relecture: Instant,
    vues: [Vue3d; 3],
    camera: Option<Camera>,
    couleurs: [Arc<Vec<[u8; 3]>>; 2],
}

impl Default for Simulation {
    fn default() -> Self {
        Simulation {
            runs: None,
            choisi: 0,
            index: 0,
            lecture: false,
            dernier_pas: Instant::now(),
            derniere_relecture: Instant::now(),
            vues: Default::default(),
            camera: None,
            couleurs: [Arc::new(vec![COULEUR_SIMULEE]), Arc::new(vec![COULEUR_OBSERVEE])],
        }
    }
}

/// Secondes depuis l'époque d'une date ISO 8601 `AAAA-MM-JJTHH:MM:SS` (fuseau ignoré, supposé identique).
fn secondes_iso(texte: &str) -> Option<i64> {
    let n = |a: usize, b: usize| texte.get(a..b)?.parse::<i64>().ok();
    let (an, mois, jour) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, m, s) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    // Jours depuis 1970 (algorithme des jours civils de H. Hinnant).
    let (y, mo) = if mois <= 2 { (an - 1, mois + 9) } else { (an, mois - 3) };
    let ere = y.div_euclid(400);
    let ya = y - ere * 400;
    let jour_annee = (153 * mo + 2) / 5 + jour - 1;
    let jours = ere * 146_097 + ya * 365 + ya / 4 - ya / 100 + jour_annee - 719_468;
    Some(jours * 86_400 + h * 3600 + m * 60 + s)
}

fn duree(secondes: i64) -> String {
    let (h, m, s) = (secondes / 3600, secondes / 60 % 60, secondes % 60);
    if h > 0 {
        format!("{h} h {m:02} min")
    } else {
        format!("{m} min {s:02} s")
    }
}

/// Temps écoulé et restant estimé, d'après `started`, `updated` et l'avancement.
fn temps(etat: &EtatRun) -> Option<(i64, Option<i64>)> {
    let ecoule = secondes_iso(etat.updated.as_deref()?)? - secondes_iso(etat.started.as_deref()?)?;
    let restant = etat
        .avancement()
        .filter(|&p| p > 0.0 && etat.state == "running")
        .map(|p| (ecoule as f64 * (1.0 - p) / p).round() as i64);
    Some((ecoule.max(0), restant))
}

impl Simulation {
    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        base: &mut Base,
        jeu: &JeuBids,
        sujet: usize,
        session: usize,
        derivees: &Path,
        chemins: &CheminsSession,
        rendu: Option<&RenderState>,
        curseur: &mut PointMonde,
    ) {
        let runs = self.runs.get_or_insert_with(|| {
            lister_runs(&chemins.simulations())
                .into_iter()
                .map(|(l, d)| Run::lire(l, d))
                .collect()
        });
        if self.derniere_relecture.elapsed() >= RELECTURE {
            runs.iter_mut().for_each(Run::relire);
            self.derniere_relecture = Instant::now();
        }
        if runs.iter().any(Run::en_cours) {
            ui.ctx().request_repaint_after(RELECTURE);
        }
        if runs.is_empty() {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let texte = format!("Aucun run gyroxide dans {}", chemins.simulations().display());
                    ui.label(RichText::new(texte).color(theme::TEXTE_SECONDAIRE));
                });
            });
            return;
        }
        self.choisi = self.choisi.min(runs.len() - 1);
        let run = &runs[self.choisi];
        let pas = run.pas().to_vec();
        self.index = self.index.min(pas.len().saturating_sub(1));

        // Surfaces : état initial, pas courant (et le suivant, lu d'avance pendant la lecture), observée.
        let surface = |base: &mut Base, i: usize| -> Statut<Maillage> {
            match pas.get(i) {
                Some(p) => base.surface(&run.dossier.join(&p.file)),
                None => Statut::Absent,
            }
        };
        let initiale = surface(base, 0);
        let courante = surface(base, self.index);
        if self.lecture {
            let _ = surface(base, self.index + 1);
        }
        let observee = run.hemisphere().and_then(|h| {
            let sujet = &jeu.sujets[sujet];
            let age = sujet.sessions[session].age_gestationnel;
            sujet
                .sessions
                .iter()
                .enumerate()
                .filter(|(i, s)| *i != session && s.age_gestationnel.zip(age).is_some_and(|(a, b)| a > b))
                .map(|(_, s)| CheminsSession::new(derivees, sujet, s).surface("pial", Some(h)))
                .find(|c| c.exists())
        });
        let observee = observee.map(|c| (base.surface(&c), c));

        // Lecture : pas suivant quand le courant est prêt.
        if self.lecture {
            if courante.pret().is_some() && self.dernier_pas.elapsed() >= CADENCE {
                if self.index + 1 < pas.len() {
                    self.index += 1;
                    self.dernier_pas = Instant::now();
                } else {
                    self.lecture = false;
                }
            }
            ui.ctx().request_repaint_after(CADENCE);
        }

        egui::Panel::left("simulation_runs")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.liste_runs(ui));
            });
        let runs = self.runs.as_ref().unwrap();
        let run = &runs[self.choisi];
        egui::Panel::right("simulation_courbes")
            .resizable(true)
            .default_size(320.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| courbes(ui, run, &pas, self.index));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let zone = ui.available_rect_before_wrap();
            let hauteur_frise = 64.0;
            let vues = egui::Rect::from_min_max(zone.min, egui::pos2(zone.right(), zone.bottom() - hauteur_frise));
            let largeur = (vues.width() - 4.0) / 3.0;
            let titres = ["état initial", "pas courant", "observée"];
            let statuts: [Option<&Statut<Maillage>>; 3] =
                [Some(&initiale), Some(&courante), observee.as_ref().map(|(s, _)| s)];
            for (k, (vue, statut)) in self.vues.iter_mut().zip(statuts).enumerate() {
                let case = egui::Rect::from_min_size(
                    vues.min + Vec2::new(k as f32 * (largeur + 2.0), 0.0),
                    Vec2::new(largeur, vues.height()),
                );
                let objets: Vec<Objet3d> = statut
                    .and_then(Statut::pret)
                    .map(|m| Objet3d {
                        maillage: m.clone(),
                        couleurs: self.couleurs[usize::from(k == 2)].clone(),
                        visible: true,
                    })
                    .into_iter()
                    .collect();
                match (rendu, objets.is_empty()) {
                    (Some(rs), false) => {
                        // Caméra partagée : chaque vue part de la caméra commune, et la vue manipulée la met à jour.
                        if let Some(c) = self.camera {
                            vue.camera = Some(c);
                        }
                        vue.ui(ui, case, rs, &objets, curseur);
                        if vue.camera != self.camera {
                            self.camera = vue.camera;
                        }
                    }
                    _ => {
                        ui.painter().rect_filled(case, 0.0, theme::FOND);
                        let texte = match statut {
                            None => "pas de session ultérieure avec une surface piale",
                            Some(Statut::EnCours) => "lecture…",
                            Some(Statut::Erreur(_)) => "surface illisible",
                            _ if rendu.is_none() => "rendu 3D indisponible",
                            _ => "absente",
                        };
                        ui.painter().text(
                            case.center(),
                            egui::Align2::CENTER_CENTER,
                            texte,
                            egui::FontId::proportional(13.0),
                            theme::TEXTE_SECONDAIRE,
                        );
                    }
                }
                ui.painter().text(
                    case.left_top() + Vec2::splat(6.0),
                    egui::Align2::LEFT_TOP,
                    titres[k],
                    egui::FontId::proportional(13.0),
                    theme::TEXTE_SECONDAIRE,
                );
                if let Some(Statut::Erreur(e)) = statut {
                    ui.painter().text(
                        case.left_bottom() + Vec2::new(6.0, -6.0),
                        egui::Align2::LEFT_BOTTOM,
                        e,
                        egui::FontId::proportional(11.0),
                        theme::ERREUR,
                    );
                }
            }
            if let Some(m) = courante.pret() {
                let (a, b) = m.boite();
                let etendue = (0..3).map(|c| b[c] - a[c]).fold(0.0, f32::max);
                if etendue < ETENDUE_MINIMALE_MM {
                    ui.painter().text(
                        egui::pos2(vues.center().x, vues.bottom() - 8.0),
                        egui::Align2::CENTER_BOTTOM,
                        "coordonnées normalisées : surfaces hors du repère de l'image (gyroxide#2)",
                        egui::FontId::proportional(12.0),
                        theme::ACCENT,
                    );
                }
            }
            let frise = egui::Rect::from_min_max(egui::pos2(zone.left(), vues.bottom() + 6.0), zone.max);
            let mut enfant = ui.new_child(egui::UiBuilder::new().max_rect(frise));
            let etat = run_etat(self.runs.as_ref().unwrap(), self.choisi).cloned();
            self.frise(&mut enfant, etat.as_ref(), &pas, &courante);
        });
    }

    fn liste_runs(&mut self, ui: &mut Ui) {
        let Some(runs) = &self.runs else { return };
        ui.strong(format!("Runs ({})", runs.len()));
        let mut choix = None;
        for (i, r) in runs.iter().enumerate() {
            ui.horizontal(|ui| {
                let (icone, couleur) = match r.etat.lu().map(|e| e.state.as_str()) {
                    Some("done") => ("✓", theme::TERMINE),
                    Some("running") => ("◔", theme::ACCENT),
                    Some("failed") => ("⚠", theme::ERREUR),
                    _ if r.etat.erreur().is_some() => ("⚠", theme::ERREUR),
                    _ => ("○", theme::TEXTE_SECONDAIRE),
                };
                ui.label(RichText::new(icone).color(couleur));
                if ui
                    .selectable_label(i == self.choisi, RichText::new(&r.label).monospace())
                    .clicked()
                {
                    choix = Some(i);
                }
                if let Some(p) = r.etat.lu().and_then(EtatRun::avancement) {
                    ui.label(
                        RichText::new(format!("{:.0} %", p * 100.0))
                            .small()
                            .color(theme::TEXTE_SECONDAIRE),
                    );
                }
            });
        }
        if let Some(i) = choix
            && i != self.choisi
        {
            (self.choisi, self.index, self.lecture, self.camera) = (i, 0, false, None);
            self.vues.iter_mut().for_each(Vue3d::recadrer);
        }
        let r = &runs[self.choisi];
        ui.add_space(8.0);
        if let Some(e) = r.etat.erreur().or(r.sequence.erreur()) {
            ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
        }
        if let Some(e) = r.etat.lu()
            && let Some(m) = &e.message
        {
            ui.label(RichText::new(m).color(if e.state == "failed" {
                theme::ERREUR
            } else {
                theme::TEXTE_SECONDAIRE
            }));
        }
        ui.strong("Paramètres (config.toml)");
        match &r.config {
            Some(texte) => {
                egui::ScrollArea::vertical()
                    .id_salt("config")
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        ui.label(RichText::new(texte).monospace().small());
                    });
            }
            None => {
                ui.label(RichText::new("absent").color(theme::TEXTE_SECONDAIRE));
            }
        }
    }

    /// Frise temporelle : lecture / pause, curseur de pas, temps et âge gestationnel, progression d'un run en cours.
    fn frise(&mut self, ui: &mut Ui, etat: Option<&EtatRun>, pas: &[PasSequence], courante: &Statut<Maillage>) {
        if pas.is_empty() {
            ui.label(RichText::new("sequence.tsv absent ou vide").color(theme::TEXTE_SECONDAIRE));
            return;
        }
        ui.horizontal(|ui| {
            let bouton = if self.lecture { "Pause" } else { "Lire" };
            if ui.button(bouton).on_hover_text("lecture / pause").clicked() {
                if !self.lecture && self.index + 1 >= pas.len() {
                    self.index = 0;
                }
                self.lecture = !self.lecture;
                self.dernier_pas = Instant::now();
            }
            icone_statut(ui, courante);
            let mut i = self.index;
            let largeur = (ui.available_width() - 260.0).max(100.0);
            ui.spacing_mut().slider_width = largeur;
            if ui
                .add(egui::Slider::new(&mut i, 0..=pas.len() - 1).show_value(false))
                .changed()
            {
                self.index = i;
                self.lecture = false;
            }
            let p = &pas[self.index];
            let mut texte = format!("pas {}", p.step);
            if let Some(t) = p.time {
                texte += &format!(" · t = {}", format!("{t:.3}").replace('.', ","));
            }
            if let Some(ga) = p.ga_weeks {
                texte += &format!(" · {} SA", format!("{ga:.1}").replace('.', ","));
            }
            ui.label(RichText::new(texte).monospace());
        });
        if let Some(e) = etat
            && e.state == "running"
            && let Some(p) = e.avancement()
        {
            ui.add(egui::ProgressBar::new(p as f32).text(format!(
                "en cours : pas {} / {}",
                e.step.unwrap_or(0),
                e.total_steps.unwrap_or(0)
            )));
        }
    }
}

fn run_etat(runs: &[Run], i: usize) -> Option<&EtatRun> {
    runs.get(i).and_then(|r| r.etat.lu())
}

/// Temps, indice de gyrification, énergie et contacts, avec le pas courant marqué.
fn courbes(ui: &mut Ui, run: &Run, pas: &[PasSequence], index: usize) {
    ui.strong("Calcul");
    match run.etat.lu() {
        Some(e) => {
            ui.label(format!("état : {}", e.state));
            if let Some((ecoule, restant)) = temps(e) {
                ui.label(format!("écoulé : {}", duree(ecoule)));
                if let Some(r) = restant {
                    ui.label(format!("restant estimé : {}", duree(r)));
                }
            }
        }
        None => {
            ui.label(RichText::new("status.json absent").color(theme::TEXTE_SECONDAIRE));
        }
    }
    // Abscisse : âge gestationnel s'il est défini partout, sinon temps du modèle, sinon pas.
    let abscisse = |p: &PasSequence| p.ga_weeks.or(p.time).unwrap_or(p.step as f64);
    let nom_abscisse = if pas.iter().all(|p| p.ga_weeks.is_some()) {
        "SA"
    } else if pas.iter().all(|p| p.time.is_some()) {
        "temps du modèle"
    } else {
        "pas"
    };
    let x_courant = pas.get(index).map(abscisse);
    let courbe = |ui: &mut Ui, titre: &str, id: &str, valeur: fn(&PasSequence) -> Option<f64>| {
        let points: Vec<[f64; 2]> = pas.iter().filter_map(|p| Some([abscisse(p), valeur(p)?])).collect();
        ui.add_space(8.0);
        ui.strong(titre);
        if points.is_empty() {
            ui.label(
                RichText::new("non renseigné dans sequence.tsv")
                    .small()
                    .color(theme::TEXTE_SECONDAIRE),
            );
            return;
        }
        Plot::new(id)
            .height(130.0)
            .x_axis_label(nom_abscisse)
            .show_background(false)
            .show(ui, |p| {
                p.line(Line::new(titre, PlotPoints::new(points)).color(theme::ACCENT));
                if let Some(x) = x_courant {
                    p.vline(VLine::new("pas courant", x).color(theme::TERMINE));
                }
            });
    };
    courbe(ui, "Indice de gyrification", "gi", |p| p.gi);
    courbe(ui, "Énergie", "energie", |p| p.energy);
    courbe(ui, "Auto-contacts", "contacts", |p| p.n_contacts.map(|n| n as f64));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_iso_et_temps_restant() {
        assert_eq!(secondes_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(secondes_iso("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(secondes_iso("2026-10-19T10:30:15Z"), Some(1_792_405_815));
        assert_eq!(secondes_iso("pas une date"), None);
        let e = EtatRun {
            state: "running".into(),
            step: Some(250),
            total_steps: Some(1000),
            started: Some("2026-10-19T10:00:00Z".into()),
            updated: Some("2026-10-19T10:10:00Z".into()),
            message: None,
        };
        assert_eq!(temps(&e), Some((600, Some(1800))));
        assert_eq!(duree(1800), "30 min 00 s");
        assert_eq!(duree(3725), "1 h 02 min");
    }
}
