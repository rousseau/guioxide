//! Nouvelle séance à partir de piles NIfTI choisies ou glissées dans la fenêtre : guioxide en fait un petit jeu BIDS
//! (`sub-X/ses-Y/anat/sub-X_ses-Y[_acq-…][_run-N]_T2w.nii.gz`) et l'ouvre sur l'onglet Examen, pour vérifier les piles
//! avant de lancer quoi que ce soit (fenêtre « Calculs », [`crate::calculs`]).
//!
//! Les piles sont copiées telles quelles (un `.nii` est seulement compressé en `.nii.gz`, que lisent les outils) :
//! guioxide n'en interprète pas le contenu ; l'en-tête est lu par `medoxide-core` pour l'affichage. Les entités du
//! nom d'origine (`sub-`, `ses-`, `acq-`, `run-`) sont reprises : le nom de la séquence (`acq-hastecor`) dit le plan
//! anatomique visé par l'opérateur, que l'affine (repère de l'appareil) ne donne pas chez le fœtus.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use egui::{RichText, Ui};
use guioxide_bids::maintenant_iso;
use guioxide_view::theme;

use crate::dialogue::Dialogue;

/// Vrai pour un fichier NIfTI (`.nii` ou `.nii.gz`).
pub fn est_nifti(chemin: &Path) -> bool {
    let nom = chemin.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    nom.ends_with(".nii") || nom.ends_with(".nii.gz")
}

/// Valeur d'une entité BIDS (`acq`, `run`…) dans un nom de fichier, si elle est une étiquette valide.
fn entite(chemin: &Path, cle: &str) -> Option<String> {
    let nom = chemin.file_name()?.to_str()?;
    let base = nom.split('.').next()?;
    base.split('_')
        .find_map(|morceau| morceau.strip_prefix(cle)?.strip_prefix('-'))
        .filter(|v| etiquette_valide(v))
        .map(str::to_owned)
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

/// Ce que l'application doit faire après la fenêtre d'import.
pub enum Demande {
    /// Jeu créé : l'ouvrir sur l'onglet Examen.
    Ouvrir(PathBuf),
}

/// Fenêtre « Nouvelle séance ».
#[derive(Default)]
pub struct Import {
    pub ouverte: bool,
    piles: Vec<Pile>,
    participant: String,
    session: String,
    age: String,
    destination: String,
    /// Vrai tant que l'utilisateur n'a pas modifié le dossier : il suit alors le participant.
    destination_auto: bool,
    erreur: Option<String>,
    choix_piles: Option<Dialogue<Vec<PathBuf>>>,
    choix_destination: Option<Dialogue>,
}

/// Dossier proposé : `~/guioxide/sub-<participant>-<date>-<heure>`.
fn destination_par_defaut(participant: &str) -> String {
    let maison = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let horodatage: String = maintenant_iso()
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T')
        .collect();
    let nom = format!(
        "sub-{participant}-{}",
        &horodatage.replace('T', "-")[..13.min(horodatage.len())]
    );
    maison.join("guioxide").join(nom).display().to_string()
}

/// Étiquette BIDS : lettres et chiffres seulement.
fn etiquette_valide(e: &str) -> bool {
    !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric())
}

impl Import {
    /// Ajoute des piles (sans doublon) et ouvre la fenêtre ; les fichiers qui ne sont pas des NIfTI sont ignorés.
    /// Participant et session sont repris du nom de la première pile qui les porte.
    pub fn ajouter(&mut self, chemins: impl IntoIterator<Item = PathBuf>) {
        for chemin in chemins {
            if est_nifti(&chemin) && !self.piles.iter().any(|p| p.chemin == chemin) {
                if self.piles.is_empty() {
                    self.participant = entite(&chemin, "sub").unwrap_or_default();
                    self.session = entite(&chemin, "ses").unwrap_or_default();
                }
                self.piles.push(Pile::lire(chemin));
            }
        }
        self.ouvrir();
    }

    pub fn ouvrir(&mut self) {
        if self.participant.is_empty() {
            self.participant = "01".to_owned();
        }
        if self.session.is_empty() {
            self.session = "01".to_owned();
        }
        if !self.ouverte && (self.destination.is_empty() || self.destination_auto) {
            self.destination = destination_par_defaut(&self.participant);
            self.destination_auto = true;
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

    /// Fenêtre d'import ; rend la demande à exécuter par l'application.
    pub fn afficher(&mut self, ctx: &egui::Context) -> Option<Demande> {
        if let Some(r) = self.choix_piles.as_ref().and_then(Dialogue::resultat) {
            self.choix_piles = None;
            self.ajouter(r.unwrap_or_default());
        }
        if let Some(r) = self.choix_destination.as_ref().and_then(Dialogue::resultat) {
            self.choix_destination = None;
            if let Some(d) = r {
                self.destination = d.display().to_string();
                self.destination_auto = false;
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
        egui::Grid::new("seance")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Participant");
                ui.horizontal(|ui| {
                    ui.label(RichText::new("sub-").monospace());
                    if ui
                        .add(egui::TextEdit::singleline(&mut self.participant).desired_width(90.0))
                        .changed()
                        && self.destination_auto
                    {
                        self.destination = destination_par_defaut(&self.participant);
                    }
                    ui.label(RichText::new("ses-").monospace());
                    ui.add(egui::TextEdit::singleline(&mut self.session).desired_width(70.0));
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
                    if ui
                        .add(egui::TextEdit::singleline(&mut self.destination).desired_width(380.0))
                        .changed()
                    {
                        self.destination_auto = false;
                    }
                    if ui.button("Parcourir…").clicked() && self.choix_destination.is_none() {
                        self.choix_destination = Some(Dialogue::dossier(ui.ctx(), "Dossier de l'étude à créer"));
                    }
                });
                ui.end_row();
            });
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "La séance s'ouvre sur l'onglet Examen pour vérifier les piles ; les calculs se lancent ensuite, \
                 étape par étape ou à la suite, depuis le bouton « Calculs ».",
            )
            .small()
            .color(theme::TEXTE_SECONDAIRE),
        );
        if let Some(e) = &self.erreur {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERREUR));
        }
        ui.add_space(8.0);
        let mut demande = None;
        ui.horizontal(|ui| {
            let bouton = egui::Button::new(RichText::new("Créer la séance").strong()).fill(theme::ACCENT_DOUX);
            let bouton = ui.add_enabled(!self.piles.is_empty(), bouton);
            if self.piles.len() == 1 {
                ui.label(
                    RichText::new("la reconstruction demandera deux piles au moins").color(theme::TEXTE_SECONDAIRE),
                );
            }
            if bouton.clicked() {
                match self.creer() {
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

    fn creer(&mut self) -> Result<PathBuf, String> {
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
        Ok(racine)
    }
}

/// Noms des piles dans le jeu : entités `acq` et `run` d'origine ; une pile dont le nom serait déjà pris reçoit un
/// numéro `run` libre.
fn noms_des_piles(participant: &str, session: &str, piles: &[PathBuf]) -> Vec<String> {
    let mut noms: Vec<String> = Vec::new();
    for source in piles {
        let acq = entite(source, "acq").map(|a| format!("_acq-{a}")).unwrap_or_default();
        let base = |run: Option<String>| {
            let run = run.map(|r| format!("_run-{r}")).unwrap_or_default();
            format!("sub-{participant}_ses-{session}{acq}{run}_T2w")
        };
        let mut nom = base(entite(source, "run"));
        let mut n = 1;
        while noms.contains(&nom) {
            nom = base(Some(n.to_string()));
            n += 1;
        }
        noms.push(nom);
    }
    noms
}

/// Crée le jeu BIDS : `dataset_description.json`, `participants.tsv`, `sub-X_sessions.tsv` (si l'âge est connu) et
/// les piles ; le chemin d'origine de chaque pile est gardé dans son `.json`.
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
    for (source, base) in piles.iter().zip(noms_des_piles(participant, session, piles)) {
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
    fn entites_du_nom_d_origine() {
        let p = Path::new("/g/sub-str002_ses-01_acq-hastesag_run-2_T2w.nii.gz");
        assert_eq!(entite(p, "sub").as_deref(), Some("str002"));
        assert_eq!(entite(p, "ses").as_deref(), Some("01"));
        assert_eq!(entite(p, "acq").as_deref(), Some("hastesag"));
        assert_eq!(entite(p, "run").as_deref(), Some("2"));
        assert_eq!(entite(Path::new("/g/axial.nii"), "acq"), None);
    }

    #[test]
    fn noms_sans_doublon() {
        let piles = [
            PathBuf::from("/g/sub-a_ses-1_acq-hasteax_T2w.nii.gz"),
            PathBuf::from("/g/sub-a_ses-1_acq-hastesag_run-1_T2w.nii.gz"),
            PathBuf::from("/g/sub-a_ses-1_acq-hastesag_run-2_T2w.nii.gz"),
            PathBuf::from("/g/axial.nii"),
            PathBuf::from("/h/axial.nii"),
        ];
        assert_eq!(
            noms_des_piles("str002", "01", &piles),
            [
                "sub-str002_ses-01_acq-hasteax_T2w",
                "sub-str002_ses-01_acq-hastesag_run-1_T2w",
                "sub-str002_ses-01_acq-hastesag_run-2_T2w",
                "sub-str002_ses-01_T2w",
                "sub-str002_ses-01_run-1_T2w",
            ]
        );
    }

    #[test]
    fn jeu_cree_est_indexe() {
        let dossier = tempfile::tempdir().unwrap();
        let sources = dossier.path().join("sources");
        std::fs::create_dir_all(&sources).unwrap();
        let a = sources.join("sub-x_acq-ax_T2w.nii.gz");
        let b = sources.join("coronal.nii");
        std::fs::write(&a, [0x1f, 0x8b, 0, 0]).unwrap();
        std::fs::write(&b, b"pas vraiment un nifti").unwrap();
        let racine = dossier.path().join("etude");
        creer_jeu(&racine, "01", "02", Some(31.5), &[a, b.clone()]).unwrap();
        let anat = racine.join("sub-01/ses-02/anat");
        assert!(anat.join("sub-01_ses-02_acq-ax_T2w.nii.gz").is_file());
        let compresse = std::fs::read(anat.join("sub-01_ses-02_T2w.nii.gz")).unwrap();
        assert_eq!(&compresse[..2], &[0x1f, 0x8b]);
        let json = std::fs::read_to_string(anat.join("sub-01_ses-02_T2w.json")).unwrap();
        assert!(json.contains("coronal.nii"));
        let jeu = guioxide_bids::indexer(&racine).unwrap();
        assert_eq!(jeu.sujets.len(), 1);
        let session = &jeu.sujets[0].sessions[0];
        assert_eq!(session.age_gestationnel, Some(31.5));
        assert_eq!(session.piles.len(), 2);
    }

    #[test]
    fn etiquettes_bids() {
        assert!(etiquette_valide("01"));
        assert!(etiquette_valide("abc2"));
        assert!(!etiquette_valide(""));
        assert!(!etiquette_valide("sub-01"));
        assert!(!etiquette_valide("0 1"));
    }
}
