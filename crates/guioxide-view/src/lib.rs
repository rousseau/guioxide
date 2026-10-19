//! `guioxide-view` : apparence et widgets de visualisation de guioxide.
//!
//! Thème, polices, pastilles d'état, vues de coupe et triplanaire. La vue 3D arrive au jalon 5.

pub mod coupe;
pub mod palettes;
pub mod polices;
pub mod theme;
pub mod vue3d;

use egui::{RichText, Ui};
use guioxide_bids::EtatEtape;

/// Icône et libellé d'un état : l'état ne repose jamais sur la couleur seule.
pub fn symbole_etat(etat: &EtatEtape) -> (&'static str, &'static str) {
    match etat {
        EtatEtape::NonFaite => ("○", "non fait"),
        EtatEtape::EnCours => ("◔", "en cours"),
        EtatEtape::Terminee => ("✓", "terminé"),
        EtatEtape::Erreur(_) => ("⚠", "erreur"),
    }
}

/// Icône d'état colorée, avec le libellé (et le message d'erreur) en infobulle.
pub fn pastille_etat(ui: &mut Ui, etat: &EtatEtape) {
    let (icone, libelle) = symbole_etat(etat);
    let couleur = match etat {
        EtatEtape::NonFaite => theme::TEXTE_SECONDAIRE,
        EtatEtape::EnCours => theme::ACCENT,
        EtatEtape::Terminee => theme::TERMINE,
        EtatEtape::Erreur(_) => theme::ERREUR,
    };
    let reponse = ui.label(RichText::new(icone).color(couleur));
    match etat {
        EtatEtape::Erreur(message) => reponse.on_hover_text(format!("{libelle} : {message}")),
        _ => reponse.on_hover_text(libelle),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_etat_a_une_icone_et_un_libelle_distincts() {
        let etats = [
            EtatEtape::NonFaite,
            EtatEtape::EnCours,
            EtatEtape::Terminee,
            EtatEtape::Erreur("x".into()),
        ];
        let symboles: Vec<_> = etats.iter().map(symbole_etat).collect();
        for (i, a) in symboles.iter().enumerate() {
            for b in &symboles[i + 1..] {
                assert_ne!(a.0, b.0);
                assert_ne!(a.1, b.1);
            }
        }
    }
}
