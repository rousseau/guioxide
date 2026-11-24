//! Nouvelle séance à partir de piles NIfTI choisies ou glissées dans la fenêtre : guioxide en fait un petit jeu BIDS
//! (`sub-X/ses-Y/anat/sub-X_ses-Y_run-N_T2w.nii.gz`), puis lance la chaîne de calcul ([`crate::chaine`]).
//!
//! Les piles sont copiées telles quelles (un `.nii` est seulement compressé en `.nii.gz`, que lisent les outils) :
//! guioxide n'en interprète pas le contenu ; l'en-tête est lu par `medoxide-core` pour l'affichage.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use egui::{Color32, RichText, Ui};
use guioxide_bids::maintenant_iso;
use guioxide_view::theme;

use crate::chaine::{self, Chaine, Etape, EtatEtape, Outil};
use crate::dialogue::Dialogue;

/// Vrai pour un fichier NIfTI (`.nii` ou `.nii.gz`).
pub fn est_nifti(chemin: &Path) -> bool {
    let nom = chemin.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    nom.ends_with(".nii") || nom.ends_with(".nii.gz")
}

/// Une pile choisie, avec ce qu'en dit l'en-tête.
struct Pile {
    chemin: PathBuf,
    /// `256 × 256 × 40 · 0,7 × 0,7 × 3,0 mm`, ou l'erreur de lecture.
    resume: Result<String, String>,
}

impl Pile {
    fn lire(chemin: PathBuf) -> Pile {
        let resume = guioxide_data::lire_geometrie(&chemin)
            .map(|(dims, affine)| {
                let taille = |j: usize| (0..3).map(|i| affine.0[i][j].powi(2)).sum::<f64>().sqrt();
                format!(
                    "{} × {} × {} · {:.2} × {:.2} × {:.2} mm",
                    dims[0],
                    dims[1],
                    dims[2],
                    taille(0),
                    taille(1),
                    taille(2)
                )
                .replace('.', ",")
            })
            .map_err(|e| e.to_string());
        Pile { chemin, resume }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mouvement {
    S2s,
    S2v,
}

/// Ce que l'application doit faire après la fenêtre d'import.
pub enum Demande {
    /// Jeu créé : l'ouvrir et suivre la chaîne lancée.
    Ouvrir(PathBuf),
}

/// Fenêtre « Nouvelle séance » et chaîne en cours.
pub struct Import {
    pub ouverte: bool,
    piles: Vec<Pile>,
    participant: String,
    session: String,
    age: String,
    destination: String,
    mouvement: Mouvement,
    simulation: bool,
    pas: u32,
    erreur: Option<String>,
    choix_piles: Option<Dialogue<Vec<PathBuf>>>,
    choix_destination: Option<Dialogue>,
    pub chaine: Option<Chaine>,
    /// Fenêtre de suivi de la chaîne affichée.
    pub suivi: bool,
}

impl Default for Import {
    fn default() -> Self {
        Import {
            ouverte: false,
            piles: Vec::new(),
            participant: "01".to_owned(),
            session: "01".to_owned(),
            age: String::new(),
            destination: String::new(),
            mouvement: Mouvement::S2s,
            simulation: true,
            pas: 2000,
            erreur: None,
            choix_piles: None,
            choix_destination: None,
            chaine: None,
            suivi: false,
        }
    }
}

/// Dossier proposé : `~/guioxide/<date>-<heure>` (un nouveau à chaque séance).
fn destination_par_defaut() -> String {
    let maison = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let horodatage: String = maintenant_iso()
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T')
        .collect();
    maison
        .join("guioxide")
        .join(horodatage.replace('T', "-"))
        .display()
        .to_string()
}

/// Étiquette BIDS : lettres et chiffres seulement.
fn etiquette_valide(e: &str) -> bool {
    !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric())
}

impl Import {
    /// Ajoute des piles (sans doublon) et ouvre la fenêtre ; les fichiers qui ne sont pas des NIfTI sont ignorés.
    pub fn ajouter(&mut self, chemins: impl IntoIterator<Item = PathBuf>) {
        for chemin in chemins {
            if est_nifti(&chemin) && !self.piles.iter().any(|p| p.chemin == chemin) {
                self.piles.push(Pile::lire(chemin));
            }
        }
        self.ouvrir();
    }

    pub fn ouvrir(&mut self) {
        if !self.ouverte && self.destination.is_empty() {
            self.destination = destination_par_defaut();
        }
        self.ouverte = true;
    }

    pub fn choisir_piles(&mut self, ctx: &egui::Context) {
        if self.choix_piles.is_none() {
            self.choix_piles = Some(Dialogue::fichiers(
                ctx,
                "Piles T2w de la séance",
                "NIfTI",
                &["nii", "gz"],
            ));
        }
        self.ouvrir();
    }

    pub fn en_cours(&self) -> bool {
        self.chaine.as_ref().is_some_and(|c| !c.finie())
    }

    /// Fenêtres d'import et de suivi ; rend la demande à exécuter par l'application.
    pub fn afficher(&mut self, ctx: &egui::Context) -> Option<Demande> {
        if let Some(r) = self.choix_piles.as_ref().and_then(Dialogue::resultat) {
            self.choix_piles = None;
            self.ajouter(r.unwrap_or_default());
        }
        if let Some(r) = self.choix_destination.as_ref().and_then(Dialogue::resultat) {
            self.choix_destination = None;
            if let Some(d) = r {
                self.destination = d.display().to_string();
            }
        }
        let mut demande = None;
        let mut ouverte = self.ouverte;
        egui::Window::new("Nouvelle séance à partir de piles NIfTI")
            .open(&mut ouverte)
            .default_width(640.0)
            .min_width(560.0)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .collapsible(false)
            .show(ctx, |ui| demande = self.formulaire(ui));
        self.ouverte = ouverte && demande.is_none();
        self.fenetre_suivi(ctx);
        demande
    }

    fn formulaire(&mut self, ui: &mut Ui) -> Option<Demande> {
        ui.label(
            RichText::new("Glisser des fichiers .nii ou .nii.gz dans la fenêtre, ou les choisir.")
                .color(theme::TEXTE_SECONDAIRE),
        );
        ui.add_space(4.0);
        let mut retirer = None;
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (i, pile) in self.piles.iter().enumerate() {
                let nom = pile.chemin.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                ui.horizontal(|ui| {
                    if ui.small_button("Retirer").clicked() {
                        retirer = Some(i);
                    }
                    ui.label(RichText::new(format!("run-{}", i + 1)).monospace().strong());
                    ui.add(egui::Label::new(RichText::new(nom).monospace()).truncate())
                        .on_hover_text(pile.chemin.display().to_string());
                });
                ui.horizontal(|ui| {
                    ui.add_space(62.0);
                    match &pile.resume {
                        Ok(r) => ui.label(RichText::new(r).small().color(theme::TEXTE_SECONDAIRE)),
                        Err(e) => ui.label(RichText::new(format!("⚠ {e}")).small().color(theme::ERREUR)),
                    };
                });
            }
        });
        if let Some(i) = retirer {
            self.piles.remove(i);
        }
        if ui.button("Ajouter des piles…").clicked() {
            self.choisir_piles(ui.ctx());
        }
        ui.add_space(8.0);
        ui.separator();
        egui::Grid::new("seance").num_columns(2).show(ui, |ui| {
            ui.label("Participant");
            ui.horizontal(|ui| {
                ui.label(RichText::new("sub-").monospace());
                ui.add(egui::TextEdit::singleline(&mut self.participant).desired_width(80.0));
                ui.label(RichText::new("ses-").monospace());
                ui.add(egui::TextEdit::singleline(&mut self.session).desired_width(80.0));
            });
            ui.end_row();
            ui.label("Âge gestationnel");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.age)
                        .hint_text("facultatif")
                        .desired_width(80.0),
                );
                ui.label("SA");
            });
            ui.end_row();
            ui.label("Dossier de l'étude");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.destination).desired_width(380.0));
                if ui.button("Parcourir…").clicked() && self.choix_destination.is_none() {
                    self.choix_destination = Some(Dialogue::dossier(ui.ctx(), "Dossier de l'étude à créer"));
                }
            });
            ui.end_row();
            ui.label("Reconstruction");
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut self.mouvement,
                    Mouvement::S2s,
                    "coupe-coupe (s2s, ~10 min sur le Mac)",
                );
                ui.radio_value(&mut self.mouvement, Mouvement::S2v, "coupe-volume (s2v, plus long)");
            });
            ui.end_row();
            ui.label("Simulation");
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.simulation, "gyroxide, hémisphère gauche");
                ui.add_enabled(
                    self.simulation,
                    egui::DragValue::new(&mut self.pas).range(10..=100_000).suffix(" pas"),
                );
            });
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Étapes : medx fetalbet → medx svr recon → medx bounti → surfx run → gyroxide mesh → gyroxide run",
            )
            .small()
            .color(theme::TEXTE_SECONDAIRE),
        );
        ui.add_space(6.0);
        let outils = self.outils();
        for (outil, trouve) in &outils {
            let (texte, couleur) = match trouve {
                Some(chemin) => (
                    format!("{} : {}", outil.binaire(), chemin.display()),
                    theme::TEXTE_SECONDAIRE,
                ),
                None => (
                    format!(
                        "{} introuvable (PATH, ~/.cargo/bin, ~/Exp/oxide/…/target/release, ou {})",
                        outil.binaire(),
                        match outil {
                            Outil::Medx => "GUIOXIDE_MEDX",
                            Outil::Surfx => "GUIOXIDE_SURFX",
                            Outil::Gyroxide => "GUIOXIDE_GYROXIDE",
                        }
                    ),
                    theme::ERREUR,
                ),
            };
            ui.label(RichText::new(texte).small().monospace().color(couleur));
        }
        if let Some(e) = &self.erreur {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
        }
        ui.add_space(8.0);
        let pret = self.piles.len() >= 2 && !self.en_cours();
        let mut demande = None;
        ui.horizontal(|ui| {
            let bouton = ui.add_enabled(
                pret,
                egui::Button::new(RichText::new("Créer la séance et lancer").strong()),
            );
            if self.piles.len() < 2 {
                ui.label(RichText::new("deux piles au moins").color(theme::TEXTE_SECONDAIRE));
            } else if self.en_cours() {
                ui.label(RichText::new("une chaîne est déjà en cours").color(theme::TEXTE_SECONDAIRE));
            }
            if bouton.clicked() {
                match self.creer_et_lancer(ui.ctx(), &outils) {
                    Ok(racine) => {
                        self.erreur = None;
                        self.piles.clear();
                        self.destination.clear();
                        demande = Some(Demande::Ouvrir(racine));
                    }
                    Err(e) => self.erreur = Some(e),
                }
            }
        });
        demande
    }

    fn outils(&self) -> Vec<(Outil, Option<PathBuf>)> {
        Outil::TOUS
            .into_iter()
            .filter(|o| self.simulation || *o != Outil::Gyroxide)
            .map(|o| (o, chaine::trouver(o)))
            .collect()
    }

    fn creer_et_lancer(&mut self, ctx: &egui::Context, outils: &[(Outil, Option<PathBuf>)]) -> Result<PathBuf, String> {
        let (participant, session) = (self.participant.trim().to_owned(), self.session.trim().to_owned());
        if !etiquette_valide(&participant) || !etiquette_valide(&session) {
            return Err("participant et session : lettres et chiffres seulement (sans « sub- » ni « ses- »)".into());
        }
        let age = match self.age.trim().replace(',', ".") {
            a if a.is_empty() => None,
            a => Some(
                a.parse::<f64>()
                    .ok()
                    .filter(|x| (10.0..=45.0).contains(x))
                    .ok_or("âge gestationnel : un nombre de semaines, par exemple 31,5")?,
            ),
        };
        if let Some(pile) = self.piles.iter().find(|p| p.resume.is_err()) {
            return Err(format!("pile illisible : {}", pile.chemin.display()));
        }
        let binaire = |o: Outil| {
            outils
                .iter()
                .find(|(x, _)| *x == o)
                .and_then(|(_, c)| c.clone())
                .ok_or(format!("{} introuvable", o.binaire()))
        };
        let medx = binaire(Outil::Medx)?;
        let surfx = binaire(Outil::Surfx)?;
        let gyroxide = if self.simulation {
            Some((
                binaire(Outil::Gyroxide)?,
                chaine::trouver_tetgen()
                    .ok_or("TetGen introuvable (gyroxide mesh) : GYROXIDE_TETGEN ou ~/.local/bin")?,
            ))
        } else {
            None
        };
        let racine = PathBuf::from(self.destination.trim());
        if racine.as_os_str().is_empty() {
            return Err("dossier de l'étude à indiquer".into());
        }
        if racine.read_dir().is_ok_and(|mut d| d.next().is_some()) {
            return Err(format!(
                "{} existe et n'est pas vide : choisir un nouveau dossier",
                racine.display()
            ));
        }
        let piles: Vec<PathBuf> = self.piles.iter().map(|p| p.chemin.clone()).collect();
        creer_jeu(&racine, &participant, &session, age, &piles)?;

        let racine_texte = racine.display().to_string();
        let commun = |sous_commandes: &[&str]| -> Vec<String> {
            let mut a: Vec<String> = sous_commandes.iter().map(|s| s.to_string()).collect();
            a.extend(
                [
                    "--bids",
                    &racine_texte,
                    "--participant",
                    &participant,
                    "--session",
                    &session,
                ]
                .map(str::to_owned),
            );
            a
        };
        let etape = |nom: &str, programme: &Path, arguments: Vec<String>| Etape {
            nom: nom.to_owned(),
            programme: programme.to_path_buf(),
            arguments,
        };
        let mouvement = match self.mouvement {
            Mouvement::S2s => "s2s",
            Mouvement::S2v => "s2v",
        };
        let mut svr = commun(&["svr", "recon"]);
        svr.extend(["--motion", mouvement, "--alpha", "0.05", "--device", "cpu"].map(str::to_owned));
        let mut etapes = vec![
            etape("medx fetalbet", &medx, commun(&["fetalbet"])),
            etape("medx svr", &medx, svr),
            etape("medx bounti", &medx, commun(&["bounti"])),
            etape("surfx run", &surfx, commun(&["run"])),
        ];
        if let Some((gyroxide, tetgen)) = gyroxide {
            let mut mesh = commun(&["mesh"]);
            mesh.extend([
                "--hemi".to_owned(),
                "L".to_owned(),
                "--tetgen".to_owned(),
                tetgen.display().to_string(),
            ]);
            let mut run = commun(&["run"]);
            run.extend(
                [
                    "--label",
                    "guioxide",
                    "--hemi",
                    "L",
                    "--steps",
                    &self.pas.to_string(),
                    "--uniform-growth",
                    "--no-midplane",
                ]
                .map(str::to_owned),
            );
            etapes.push(etape("gyroxide mesh", &gyroxide, mesh));
            etapes.push(etape("gyroxide run", &gyroxide, run));
        }
        let journaux = racine.join("derivatives/guioxide/journaux");
        self.chaine = Some(Chaine::lancer(ctx, etapes, journaux)?);
        self.suivi = true;
        Ok(racine)
    }

    /// Fenêtre de suivi : état de chaque étape, fin du journal de l'étape en cours, annulation.
    fn fenetre_suivi(&mut self, ctx: &egui::Context) {
        let Some(chaine) = &self.chaine else { return };
        if !chaine.finie() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        let mut ouverte = self.suivi;
        egui::Window::new("Calculs")
            .open(&mut ouverte)
            .default_width(640.0)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -40.0])
            .show(ctx, |ui| {
                let etats = chaine.etats();
                egui::Grid::new("etapes_chaine").num_columns(3).show(ui, |ui| {
                    for (i, (etape, etat)) in chaine.etapes.iter().zip(&etats).enumerate() {
                        let (signe, couleur, detail) = match etat {
                            EtatEtape::Attente => ("·", theme::TEXTE_SECONDAIRE, String::new()),
                            EtatEtape::EnCours => (
                                "▶",
                                theme::ACCENT,
                                format!("{} s", chaine.duree_etape_courante().as_secs()),
                            ),
                            EtatEtape::Terminee(d) => ("✓", theme::TERMINE, format!("{} s", d.as_secs())),
                            EtatEtape::Echec(e) => ("✗", theme::ERREUR, e.clone()),
                            EtatEtape::Annulee => ("–", theme::TEXTE_SECONDAIRE, "non lancée".to_owned()),
                        };
                        ui.label(RichText::new(signe).monospace().color(couleur));
                        ui.label(RichText::new(&etape.nom).monospace())
                            .on_hover_text(etape.ligne_de_commande());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(detail).color(couleur));
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
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(chaine::fin_du_journal(&chaine.journal(i), 10))
                                    .small()
                                    .monospace(),
                            );
                        });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if chaine.finie() {
                        ui.label(format!("Fini en {} min", chaine.debut.elapsed().as_secs() / 60));
                    } else {
                        if ui.button("Annuler").clicked() {
                            chaine.annuler();
                        }
                        ui.label(
                            RichText::new(format!("{} min", chaine.debut.elapsed().as_secs() / 60))
                                .color(theme::TEXTE_SECONDAIRE),
                        );
                    }
                    if ui.button("Dossier des journaux").clicked() {
                        let _ = std::process::Command::new("open").arg(&chaine.journaux).spawn();
                    }
                });
            });
        self.suivi = ouverte;
    }
}

/// Crée le jeu BIDS : `dataset_description.json`, `participants.tsv`, `sub-X_sessions.tsv` (si l'âge est connu) et
/// les piles, numérotées dans l'ordre choisi ; le nom d'origine de chaque pile est gardé dans son `.json`.
fn creer_jeu(
    racine: &Path,
    participant: &str,
    session: &str,
    age: Option<f64>,
    piles: &[PathBuf],
) -> Result<(), String> {
    let erreur = |chemin: &Path, e: std::io::Error| format!("{} : {e}", chemin.display());
    let sujet = racine.join(format!("sub-{participant}"));
    let anat = sujet.join(format!("ses-{session}")).join("anat");
    std::fs::create_dir_all(&anat).map_err(|e| erreur(&anat, e))?;
    let ecrire = |chemin: PathBuf, texte: String| std::fs::write(&chemin, texte).map_err(|e| erreur(&chemin, e));
    let description = serde_json::json!({
        "Name": format!("guioxide sub-{participant}"),
        "BIDSVersion": "1.9.0",
        "DatasetType": "raw",
        "GeneratedBy": [{ "Name": "guioxide", "Version": env!("CARGO_PKG_VERSION"), "Description": "import de piles NIfTI" }],
    });
    ecrire(
        racine.join("dataset_description.json"),
        serde_json::to_string_pretty(&description).unwrap_or_default(),
    )?;
    ecrire(
        racine.join("participants.tsv"),
        format!("participant_id\nsub-{participant}\n"),
    )?;
    if let Some(age) = age {
        ecrire(
            sujet.join(format!("sub-{participant}_sessions.tsv")),
            format!("session_id\tga_weeks\nses-{session}\t{age}\n"),
        )?;
    }
    for (i, source) in piles.iter().enumerate() {
        let base = format!("sub-{participant}_ses-{session}_run-{}_T2w", i + 1);
        let cible = anat.join(format!("{base}.nii.gz"));
        let gz = source.to_string_lossy().to_lowercase().ends_with(".gz");
        if gz {
            std::fs::copy(source, &cible).map_err(|e| erreur(source, e))?;
        } else {
            let mut lecture = File::open(source).map_err(|e| erreur(source, e))?;
            let ecriture = BufWriter::new(File::create(&cible).map_err(|e| erreur(&cible, e))?);
            let mut gz = flate2::write::GzEncoder::new(ecriture, flate2::Compression::fast());
            std::io::copy(&mut lecture, &mut gz).map_err(|e| erreur(&cible, e))?;
            gz.finish().map_err(|e| erreur(&cible, e))?;
        }
        let json = serde_json::json!({ "OriginalFile": source.display().to_string() });
        ecrire(
            anat.join(format!("{base}.json")),
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnait_les_nifti() {
        assert!(est_nifti(Path::new("/a/pile.nii")));
        assert!(est_nifti(Path::new("/a/PILE.NII.GZ")));
        assert!(!est_nifti(Path::new("/a/pile.gz")));
        assert!(!est_nifti(Path::new("/a/dossier")));
    }

    #[test]
    fn jeu_cree_est_indexe() {
        let dossier = tempfile::tempdir().unwrap();
        let sources = dossier.path().join("sources");
        std::fs::create_dir_all(&sources).unwrap();
        let a = sources.join("axial.nii.gz");
        let b = sources.join("coronal.nii");
        std::fs::write(&a, [0x1f, 0x8b, 0, 0]).unwrap();
        std::fs::write(&b, b"pas vraiment un nifti").unwrap();
        let racine = dossier.path().join("etude");
        creer_jeu(&racine, "01", "02", Some(31.5), &[a, b.clone()]).unwrap();
        let anat = racine.join("sub-01/ses-02/anat");
        assert!(anat.join("sub-01_ses-02_run-1_T2w.nii.gz").is_file());
        let compresse = std::fs::read(anat.join("sub-01_ses-02_run-2_T2w.nii.gz")).unwrap();
        assert_eq!(&compresse[..2], &[0x1f, 0x8b]);
        let json = std::fs::read_to_string(anat.join("sub-01_ses-02_run-2_T2w.json")).unwrap();
        assert!(json.contains("coronal.nii"));
        let jeu = guioxide_bids::indexer(&racine).unwrap();
        assert_eq!(jeu.sujets.len(), 1);
        let session = &jeu.sujets[0].sessions[0];
        assert_eq!(session.age_gestationnel, Some(31.5));
    }

    #[test]
    fn etiquettes_bids() {
        assert!(etiquette_valide("01"));
        assert!(etiquette_valide("abc2"));
        assert!(!etiquette_valide(""));
        assert!(!etiquette_valide("sub-01"));
        assert!(!etiquette_valide("0 1"));
    }

    /// Chaîne réelle, par le même chemin que le bouton : `GUIOXIDE_PILES` (piles séparées par `:`) et
    /// `GUIOXIDE_ETUDE` (dossier à créer) ; reconstruction s2s et 20 pas de simulation.
    #[test]
    #[ignore = "outils de la suite et piles réelles"]
    fn chaine_reelle() {
        let piles = std::env::var("GUIOXIDE_PILES").expect("GUIOXIDE_PILES");
        let ctx = egui::Context::default();
        let mut import = Import::default();
        import.ajouter(piles.split(':').map(PathBuf::from));
        import.destination = std::env::var("GUIOXIDE_ETUDE").expect("GUIOXIDE_ETUDE");
        import.mouvement = Mouvement::S2s;
        import.pas = 20;
        let outils = import.outils();
        let racine = import.creer_et_lancer(&ctx, &outils).unwrap();
        let chaine = import.chaine.as_ref().unwrap();
        let mut vu = 0;
        while !chaine.finie() {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let etats = chaine.etats();
            let faites = etats.iter().filter(|e| matches!(e, EtatEtape::Terminee(_))).count();
            if faites != vu {
                vu = faites;
                eprintln!("{faites}/{} : {:?}", etats.len(), etats);
            }
        }
        let etats = chaine.etats();
        for (i, e) in etats.iter().enumerate() {
            if !matches!(e, EtatEtape::Terminee(_)) {
                eprintln!("{}", chaine::fin_du_journal(&chaine.journal(i), 30));
            }
        }
        assert!(etats.iter().all(|e| matches!(e, EtatEtape::Terminee(_))), "{etats:?}");
        let jeu = guioxide_bids::indexer(&racine).unwrap();
        let session = &jeu.sujets[0].sessions[0];
        let etats = guioxide_bids::etats_session(&jeu.derivees_etude(), &jeu.sujets[0], session);
        eprintln!("états des quatre onglets : {etats:?}");
    }
}
