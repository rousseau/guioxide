//! Lecture des volumes sur des fils d'arrière-plan, avec un cache évincé par taille (le moins récemment demandé
//! d'abord).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use crate::{VolumeLu, lire_volume};

/// État d'un volume demandé.
#[derive(Clone, Debug)]
pub enum Etat {
    EnCours,
    Pret(Arc<VolumeLu>),
    Erreur(String),
}

enum Entree {
    EnCours,
    Pret { volume: Arc<VolumeLu>, demande: u64 },
    Erreur(String),
}

type Reveil = Arc<dyn Fn() + Send + Sync>;

pub struct Chargeur {
    limite: usize,
    entrees: HashMap<PathBuf, Entree>,
    horloge: u64,
    emetteur: mpsc::Sender<(PathBuf, Result<VolumeLu, String>)>,
    recepteur: mpsc::Receiver<(PathBuf, Result<VolumeLu, String>)>,
    /// Appelé à la fin de chaque lecture : l'interface se redessine.
    reveil: Reveil,
}

impl Chargeur {
    /// `limite` : taille du cache, en octets de voxels.
    pub fn new(limite: usize, reveil: impl Fn() + Send + Sync + 'static) -> Chargeur {
        let (emetteur, recepteur) = mpsc::channel();
        Chargeur {
            limite,
            entrees: HashMap::new(),
            horloge: 0,
            emetteur,
            recepteur,
            reveil: Arc::new(reveil),
        }
    }

    /// État de `chemin` ; lance la lecture s'il n'a jamais été demandé (ou s'il a été évincé).
    pub fn demander(&mut self, chemin: &Path) -> Etat {
        self.recevoir();
        self.horloge += 1;
        let horloge = self.horloge;
        match self.entrees.get_mut(chemin) {
            Some(Entree::EnCours) => Etat::EnCours,
            Some(Entree::Pret { volume, demande }) => {
                *demande = horloge;
                Etat::Pret(volume.clone())
            }
            Some(Entree::Erreur(e)) => Etat::Erreur(e.clone()),
            None => {
                self.entrees.insert(chemin.to_path_buf(), Entree::EnCours);
                let (emetteur, reveil, chemin) = (self.emetteur.clone(), self.reveil.clone(), chemin.to_path_buf());
                std::thread::spawn(move || {
                    let lu = lire_volume(&chemin).map_err(|e| e.to_string());
                    let _ = emetteur.send((chemin, lu));
                    reveil();
                });
                Etat::EnCours
            }
        }
    }

    /// Oublie une erreur ou un volume, pour relire le fichier à la prochaine demande.
    pub fn oublier(&mut self, chemin: &Path) {
        if !matches!(self.entrees.get(chemin), Some(Entree::EnCours)) {
            self.entrees.remove(chemin);
        }
    }

    /// Octets de voxels en cache.
    pub fn occupation(&self) -> usize {
        self.entrees
            .values()
            .map(|e| {
                if let Entree::Pret { volume, .. } = e {
                    volume.volume.octets()
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
                    volume: Arc::new(v),
                    demande: self.horloge,
                },
                Err(e) => Entree::Erreur(e),
            };
            self.entrees.insert(chemin.clone(), entree);
            self.evincer(&chemin);
        }
    }

    /// Évince les volumes les moins récemment demandés tant que la limite est dépassée, sauf `garde`.
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
