//! Lecture de fichiers (volumes, surfaces, champs) sur des fils d'arrière-plan, avec un cache évincé par taille (le
//! moins récemment demandé d'abord).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use crate::{Maillage, VolumeLu, lire_champ, lire_surface, lire_volume};

/// État d'un fichier demandé.
#[derive(Debug)]
pub enum Etat<T> {
    EnCours,
    Pret(Arc<T>),
    Erreur(String),
}

enum Entree<T> {
    EnCours,
    Pret { valeur: Arc<T>, demande: u64 },
    Erreur(String),
}

type Message<T> = (PathBuf, Result<T, String>);
type Reveil = Arc<dyn Fn() + Send + Sync>;

pub struct Chargeur<T> {
    limite: usize,
    lire: fn(&Path) -> Result<T, String>,
    taille: fn(&T) -> usize,
    entrees: HashMap<PathBuf, Entree<T>>,
    horloge: u64,
    emetteur: mpsc::Sender<Message<T>>,
    recepteur: mpsc::Receiver<Message<T>>,
    /// Appelé à la fin de chaque lecture : l'interface se redessine.
    reveil: Reveil,
}

impl Chargeur<VolumeLu> {
    /// Volumes NIfTI ; `limite` : taille du cache, en octets de voxels.
    pub fn volumes(limite: usize, reveil: impl Fn() + Send + Sync + 'static) -> Self {
        Chargeur::new(
            |c| lire_volume(c).map_err(|e| e.to_string()),
            |v| v.volume.octets(),
            limite,
            reveil,
        )
    }
}

impl Chargeur<Maillage> {
    /// Surfaces GIfTI ; `limite` en octets de sommets et de faces.
    pub fn surfaces(limite: usize, reveil: impl Fn() + Send + Sync + 'static) -> Self {
        let taille = |m: &Maillage| (m.sommets.len() + m.faces.len()) * 12;
        Chargeur::new(|c| lire_surface(c).map_err(|e| e.to_string()), taille, limite, reveil)
    }
}

impl Chargeur<Vec<f32>> {
    /// Champs par sommet GIfTI (`.shape.gii`) ; `limite` en octets.
    pub fn champs(limite: usize, reveil: impl Fn() + Send + Sync + 'static) -> Self {
        Chargeur::new(
            |c| lire_champ(c).map_err(|e| e.to_string()),
            |v| v.len() * 4,
            limite,
            reveil,
        )
    }
}

impl<T: Send + Sync + 'static> Chargeur<T> {
    /// `lire` lit un fichier sur un fil d'arrière-plan ; `taille` en donne l'encombrement (octets) pour le cache.
    pub fn new(
        lire: fn(&Path) -> Result<T, String>,
        taille: fn(&T) -> usize,
        limite: usize,
        reveil: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let (emetteur, recepteur) = mpsc::channel();
        let reveil: Reveil = Arc::new(reveil);
        Chargeur {
            limite,
            lire,
            taille,
            entrees: HashMap::new(),
            horloge: 0,
            emetteur,
            recepteur,
            reveil,
        }
    }

    /// État de `chemin` ; lance la lecture s'il n'a jamais été demandé (ou s'il a été évincé).
    pub fn demander(&mut self, chemin: &Path) -> Etat<T> {
        self.recevoir();
        self.horloge += 1;
        let horloge = self.horloge;
        match self.entrees.get_mut(chemin) {
            Some(Entree::EnCours) => Etat::EnCours,
            Some(Entree::Pret { valeur, demande }) => {
                *demande = horloge;
                Etat::Pret(valeur.clone())
            }
            Some(Entree::Erreur(e)) => Etat::Erreur(e.clone()),
            None => {
                self.entrees.insert(chemin.to_path_buf(), Entree::EnCours);
                let (emetteur, reveil, lire) = (self.emetteur.clone(), self.reveil.clone(), self.lire);
                let chemin = chemin.to_path_buf();
                std::thread::spawn(move || {
                    let lu = lire(&chemin);
                    let _ = emetteur.send((chemin, lu));
                    reveil();
                });
                Etat::EnCours
            }
        }
    }

    /// Oublie une erreur ou une valeur, pour relire le fichier à la prochaine demande.
    pub fn oublier(&mut self, chemin: &Path) {
        if !matches!(self.entrees.get(chemin), Some(Entree::EnCours)) {
            self.entrees.remove(chemin);
        }
    }

    /// Octets en cache.
    pub fn occupation(&self) -> usize {
        let taille = self.taille;
        self.entrees
            .values()
            .map(|e| {
                if let Entree::Pret { valeur, .. } = e {
                    taille(valeur)
                } else {
                    0
                }
            })
            .sum()
    }

    fn recevoir(&mut self) {
        while let Ok((chemin, lu)) = self.recepteur.try_recv() {
            let entree = match lu {
                Ok(v) => Entree::Pret {
                    valeur: Arc::new(v),
                    demande: self.horloge,
                },
                Err(e) => Entree::Erreur(e),
            };
            self.entrees.insert(chemin.clone(), entree);
            self.evincer(&chemin);
        }
    }

    /// Évince les valeurs les moins récemment demandées tant que la limite est dépassée, sauf `garde`.
    fn evincer(&mut self, garde: &Path) {
        while self.occupation() > self.limite {
            let ancien = self
                .entrees
                .iter()
                .filter_map(|(c, e)| match e {
                    Entree::Pret { demande, .. } if c != garde => Some((*demande, c.clone())),
                    _ => None,
                })
                .min();
            match ancien {
                Some((_, c)) => self.entrees.remove(&c),
                None => break,
            };
        }
    }
}
