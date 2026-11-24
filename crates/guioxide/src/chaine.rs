//! Lancement de la chaîne medx → surfx → gyroxide sur une séance, en sous-processus (`docs/architecture.md`,
//! principe 3) : chaque étape est un programme séparé, sa sortie va dans un journal, l'interface relit les fichiers
//! produits. Première version, pour essayer la suite de bout en bout ; l'exécuteur complet (file d'attente,
//! formulaires tirés de `--describe`, tâches rattachées au redémarrage) est le jalon P2.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Les trois programmes de la suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outil {
    Medx,
    Surfx,
    Gyroxide,
}

impl Outil {
    pub fn binaire(self) -> &'static str {
        match self {
            Outil::Medx => "medx",
            Outil::Surfx => "surfx",
            Outil::Gyroxide => "gyroxide",
        }
    }

    fn depot(self) -> &'static str {
        match self {
            Outil::Medx => "medoxide",
            Outil::Surfx => "surfoxide",
            Outil::Gyroxide => "gyroxide",
        }
    }

    fn variable(self) -> &'static str {
        match self {
            Outil::Medx => "GUIOXIDE_MEDX",
            Outil::Surfx => "GUIOXIDE_SURFX",
            Outil::Gyroxide => "GUIOXIDE_GYROXIDE",
        }
    }
}

/// Dossiers ajoutés au `PATH` des outils et parcourus pour les trouver : une application lancée depuis le Dock
/// n'hérite pas du `PATH` du shell.
fn dossiers_supplementaires() -> Vec<PathBuf> {
    let mut dossiers = Vec::new();
    if let Some(maison) = std::env::var_os("HOME").map(PathBuf::from) {
        dossiers.push(maison.join(".cargo/bin"));
        dossiers.push(maison.join(".local/bin"));
    }
    dossiers.push(PathBuf::from("/opt/homebrew/bin"));
    dossiers.push(PathBuf::from("/usr/local/bin"));
    dossiers
}

/// `PATH` des sous-processus : celui de l'application, complété.
fn path_outils() -> std::ffi::OsString {
    let mut dossiers: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for d in dossiers_supplementaires() {
        if !dossiers.contains(&d) {
            dossiers.push(d);
        }
    }
    std::env::join_paths(dossiers).unwrap_or_default()
}

/// Trouve un outil : variable `GUIOXIDE_MEDX` (… `_SURFX`, `_GYROXIDE`), puis le `PATH` complété, puis
/// `target/release/` des clones de la suite (`$OXIDE_RACINE`, défaut `~/Exp/oxide/<dépôt>`).
pub fn trouver(outil: Outil) -> Option<PathBuf> {
    if let Some(chemin) = std::env::var_os(outil.variable()).map(PathBuf::from) {
        return chemin.is_file().then_some(chemin);
    }
    let path = path_outils();
    let candidats = std::env::split_paths(&path).map(|d| d.join(outil.binaire()));
    let racine = std::env::var_os("OXIDE_RACINE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Exp/oxide")));
    let clones = racine.map(|r| r.join(outil.depot()).join("target/release").join(outil.binaire()));
    candidats.chain(clones).find(|c| c.is_file())
}

/// TetGen, pour `gyroxide mesh` : même recherche que gyroxide (variable, `PATH`, `~/.local/bin`).
pub fn trouver_tetgen() -> Option<PathBuf> {
    if let Some(chemin) = std::env::var_os("GYROXIDE_TETGEN").map(PathBuf::from) {
        return chemin.is_file().then_some(chemin);
    }
    std::env::split_paths(&path_outils())
        .map(|d| d.join("tetgen"))
        .find(|c| c.is_file())
}

/// Une étape de la chaîne : un programme et ses arguments.
#[derive(Clone, Debug)]
pub struct Etape {
    pub nom: String,
    pub programme: PathBuf,
    pub arguments: Vec<String>,
    /// Clé des durées mémorisées (estimation du temps restant) et quantité de travail : piles, coupes, pas…
    pub cle: String,
    pub unites: f64,
    /// Durée prévue, si une exécution précédente (ou une mesure de référence) permet de l'estimer.
    pub prevue: Option<Duration>,
    /// Nombre total de pas, si l'outil écrit son avancement dans le journal (gyroxide run : `<pas> …` par ligne).
    pub pas: Option<u64>,
}

impl Etape {
    pub fn new(nom: &str, programme: &Path, arguments: Vec<String>) -> Etape {
        Etape {
            nom: nom.to_owned(),
            programme: programme.to_path_buf(),
            arguments,
            cle: nom.to_owned(),
            unites: 1.0,
            prevue: None,
            pas: None,
        }
    }

    pub fn ligne_de_commande(&self) -> String {
        let mut ligne = self.programme.display().to_string();
        for a in &self.arguments {
            ligne.push(' ');
            ligne.push_str(a);
        }
        ligne
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EtatEtape {
    Attente,
    EnCours,
    Terminee(Duration),
    Echec(String),
    Annulee,
}

struct Partage {
    etats: Vec<EtatEtape>,
    /// Processus en cours, pour l'annulation.
    enfant: Option<Child>,
    annuler: bool,
    /// Indice de l'étape en cours, pour le journal affiché.
    courante: Option<usize>,
    /// Nombre d'étapes terminées avec succès depuis la dernière lecture (l'interface relit alors les dérivées).
    nouvelles: usize,
    fin: bool,
}

/// Chaîne lancée : les étapes tournent l'une après l'autre sur un fil ; l'échec ou l'annulation d'une étape arrête
/// les suivantes, qui dépendent de ses fichiers.
pub struct Chaine {
    pub etapes: Vec<Etape>,
    pub journaux: PathBuf,
    pub debut: Instant,
    debut_etape: Arc<Mutex<Instant>>,
    partage: Arc<Mutex<Partage>>,
}

impl Chaine {
    pub fn lancer(ctx: &egui::Context, etapes: Vec<Etape>, journaux: PathBuf) -> Result<Chaine, String> {
        std::fs::create_dir_all(&journaux).map_err(|e| format!("{} : {e}", journaux.display()))?;
        let partage = Arc::new(Mutex::new(Partage {
            etats: vec![EtatEtape::Attente; etapes.len()],
            enfant: None,
            annuler: false,
            courante: None,
            nouvelles: 0,
            fin: false,
        }));
        let debut_etape = Arc::new(Mutex::new(Instant::now()));
        let chaine = Chaine {
            etapes: etapes.clone(),
            journaux: journaux.clone(),
            debut: Instant::now(),
            debut_etape: debut_etape.clone(),
            partage: partage.clone(),
        };
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for (i, etape) in etapes.iter().enumerate() {
                let journal = journaux.join(nom_journal(i, &etape.nom));
                *debut_etape.lock().unwrap() = Instant::now();
                let lance = {
                    let mut p = partage.lock().unwrap();
                    if p.annuler {
                        p.etats[i] = EtatEtape::Annulee;
                        continue;
                    }
                    p.courante = Some(i);
                    match demarrer(etape, &journal) {
                        Ok(enfant) => {
                            p.enfant = Some(enfant);
                            p.etats[i] = EtatEtape::EnCours;
                            true
                        }
                        Err(e) => {
                            p.etats[i] = EtatEtape::Echec(e);
                            false
                        }
                    }
                };
                ctx.request_repaint();
                let etat = if lance { attendre(&partage) } else { None };
                let mut p = partage.lock().unwrap();
                p.enfant = None;
                if let Some(etat) = etat {
                    if matches!(etat, EtatEtape::Terminee(_)) {
                        p.nouvelles += 1;
                    }
                    p.etats[i] = etat;
                }
                if !matches!(p.etats[i], EtatEtape::Terminee(_)) {
                    // Les suivantes dépendent de celle-ci.
                    for e in p.etats.iter_mut().skip(i + 1) {
                        *e = EtatEtape::Annulee;
                    }
                    break;
                }
                ctx.request_repaint();
            }
            partage.lock().unwrap().fin = true;
            ctx.request_repaint();
        });
        Ok(chaine)
    }

    /// Tue le processus en cours ; les étapes suivantes ne sont pas lancées. Les fichiers déjà écrits par l'étape
    /// interrompue restent (ils peuvent être partiels).
    pub fn annuler(&self) {
        let mut p = self.partage.lock().unwrap();
        p.annuler = true;
        if let Some(enfant) = &mut p.enfant {
            let _ = enfant.kill();
        }
    }

    pub fn etats(&self) -> Vec<EtatEtape> {
        self.partage.lock().unwrap().etats.clone()
    }

    pub fn finie(&self) -> bool {
        self.partage.lock().unwrap().fin
    }

    pub fn courante(&self) -> Option<usize> {
        self.partage.lock().unwrap().courante
    }

    pub fn duree_etape_courante(&self) -> Duration {
        self.debut_etape.lock().unwrap().elapsed()
    }

    /// Vrai si une étape s'est terminée depuis le dernier appel : les dérivées sont à relire.
    pub fn a_relire(&self) -> bool {
        std::mem::take(&mut self.partage.lock().unwrap().nouvelles) > 0
    }

    pub fn journal(&self, i: usize) -> PathBuf {
        self.journaux.join(nom_journal(i, &self.etapes[i].nom))
    }
}

fn nom_journal(i: usize, nom: &str) -> String {
    format!("{}-{}.log", i + 1, nom.replace([' ', '/'], "-"))
}

fn demarrer(etape: &Etape, journal: &Path) -> Result<Child, String> {
    let fichier = File::create(journal).map_err(|e| format!("{} : {e}", journal.display()))?;
    use std::io::Write;
    let mut entete = &fichier;
    let _ = writeln!(entete, "$ {}\n", etape.ligne_de_commande());
    let erreurs = fichier.try_clone().map_err(|e| e.to_string())?;
    Command::new(&etape.programme)
        .args(&etape.arguments)
        .env("PATH", path_outils())
        .stdin(Stdio::null())
        .stdout(Stdio::from(fichier))
        .stderr(Stdio::from(erreurs))
        .spawn()
        .map_err(|e| format!("{} : {e}", etape.programme.display()))
}

/// Attend la fin du processus en cours, sans garder le verrou (l'annulation doit pouvoir le prendre).
fn attendre(partage: &Mutex<Partage>) -> Option<EtatEtape> {
    let debut = Instant::now();
    loop {
        {
            let mut p = partage.lock().unwrap();
            let annuler = p.annuler;
            let enfant = p.enfant.as_mut()?;
            match enfant.try_wait() {
                Ok(Some(statut)) if annuler && !statut.success() => return Some(EtatEtape::Annulee),
                Ok(Some(statut)) if statut.success() => return Some(EtatEtape::Terminee(debut.elapsed())),
                Ok(Some(statut)) => return Some(EtatEtape::Echec(format!("code de sortie : {statut}"))),
                Ok(None) => {}
                Err(e) => return Some(EtatEtape::Echec(e.to_string())),
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Dernières lignes d'un journal (les barres de progression réécrites par `\r` ne gardent que leur dernier état).
pub fn fin_du_journal(chemin: &Path, lignes: usize) -> String {
    let Ok(mut f) = File::open(chemin) else {
        return String::new();
    };
    let taille = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(taille.saturating_sub(16 * 1024)));
    let mut octets = Vec::new();
    let _ = f.read_to_end(&mut octets);
    let texte = String::from_utf8_lossy(&octets);
    let toutes: Vec<&str> = texte
        .split('\n')
        .map(|l| l.rsplit('\r').find(|s| !s.trim().is_empty()).unwrap_or(""))
        .collect();
    let toutes: Vec<&str> = toutes.into_iter().filter(|l| !l.trim().is_empty()).collect();
    toutes[toutes.len().saturating_sub(lignes)..].join("\n")
}

/// Pas atteint d'après la dernière ligne du journal qui commence par un nombre (sortie de `gyroxide run`).
pub fn pas_atteint(chemin: &Path) -> Option<u64> {
    let fin = fin_du_journal(chemin, 6);
    fin.lines()
        .rev()
        .find_map(|l| l.split_whitespace().next()?.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fin_du_journal_garde_le_dernier_etat_des_barres() {
        let dossier = tempfile::tempdir().unwrap();
        let chemin = dossier.path().join("j.log");
        std::fs::write(&chemin, "a\nb\n10 %\r50 %\r100 %\nc\n").unwrap();
        assert_eq!(fin_du_journal(&chemin, 3), "b\n100 %\nc");
    }

    #[test]
    fn pas_atteint_lu_dans_le_journal() {
        let dossier = tempfile::tempdir().unwrap();
        let chemin = dossier.path().join("j.log");
        std::fs::write(
            &chemin,
            "step t\n      500    0.0007  8.9s\n     1000    0.0015  16.8s\n",
        )
        .unwrap();
        assert_eq!(pas_atteint(&chemin), Some(1000));
        std::fs::write(&chemin, "rien\n").unwrap();
        assert_eq!(pas_atteint(&chemin), None);
    }

    #[test]
    fn chaine_s_arrete_au_premier_echec() {
        let ctx = egui::Context::default();
        let dossier = tempfile::tempdir().unwrap();
        let etape = |nom: &str, code: &str| {
            Etape::new(
                nom,
                Path::new("/bin/sh"),
                vec!["-c".to_owned(), format!("echo {nom}; exit {code}")],
            )
        };
        let chaine = Chaine::lancer(
            &ctx,
            vec![etape("un", "0"), etape("deux", "3"), etape("trois", "0")],
            dossier.path().join("journaux"),
        )
        .unwrap();
        while !chaine.finie() {
            std::thread::sleep(Duration::from_millis(20));
        }
        let etats = chaine.etats();
        assert!(matches!(etats[0], EtatEtape::Terminee(_)));
        assert!(matches!(etats[1], EtatEtape::Echec(_)));
        assert_eq!(etats[2], EtatEtape::Annulee);
        assert!(fin_du_journal(&chaine.journal(0), 5).ends_with("un"));
        assert!(chaine.a_relire());
        assert!(!chaine.a_relire());
    }

    #[test]
    fn annulation_tue_le_processus() {
        let ctx = egui::Context::default();
        let dossier = tempfile::tempdir().unwrap();
        let etape = Etape::new("long", Path::new("/bin/sleep"), vec!["30".to_owned()]);
        let chaine = Chaine::lancer(&ctx, vec![etape.clone(), etape], dossier.path().to_path_buf()).unwrap();
        while chaine.etats()[0] != EtatEtape::EnCours {
            std::thread::sleep(Duration::from_millis(20));
        }
        chaine.annuler();
        let debut = Instant::now();
        while !chaine.finie() {
            assert!(debut.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(chaine.etats(), vec![EtatEtape::Annulee, EtatEtape::Annulee]);
    }
}
