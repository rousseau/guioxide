//! Résumé des sorties du SVR d'une session : qualité et mouvement par pile, calculés comme dans l'interface.
//!
//! ```text
//! cargo run --release -p guioxide-data --example svr -- <dossier anat des piles> <dossier anat de medx-svr>
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use guioxide_data::{lire_geometrie, lire_mouvement, lire_qualite, mouvement_par_coupe};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [piles, svr] = &args[..] else {
        return Err("usage : svr <dossier anat des piles> <dossier anat de medx-svr>".into());
    };
    let trouver = |suffixe: &str| -> Option<std::path::PathBuf> {
        std::fs::read_dir(svr)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.to_string_lossy().ends_with(suffixe))
    };
    let mouvement = lire_mouvement(&trouver("_desc-svr_motion.tsv").ok_or("motion.tsv absent")?)?;
    let qualite = match trouver("_desc-svr_slicequality.tsv") {
        Some(c) => Some(lire_qualite(&c)?),
        None => None,
    };
    let geometrie = |pile: &str| {
        lire_geometrie(&Path::new(piles).join(format!("{pile}.nii.gz")))
            .ok()
            .map(|(d, a)| (d, a.0))
    };
    let points = mouvement_par_coupe(&mouvement, geometrie, qualite.as_deref());

    let mut par_pile: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for p in &points {
        par_pile.entry(p.stack.as_str()).or_default().push(p);
    }
    println!("pile\tcoupes\texclues\ttranslation_moy_mm\ttranslation_max_mm\trotation_max_deg");
    for (pile, pts) in par_pile {
        let moy = pts.iter().map(|p| p.translation_mm).sum::<f64>() / pts.len() as f64;
        let max_t = pts.iter().map(|p| p.translation_mm).fold(0.0, f64::max);
        let max_r = pts.iter().map(|p| p.rotation_deg).fold(0.0, f64::max);
        let exclues = pts.iter().filter(|p| p.excluded).count();
        println!("{pile}\t{}\t{exclues}\t{moy:.4}\t{max_t:.4}\t{max_r:.4}", pts.len());
    }
    println!(
        "{} lignes de mouvement, {} coupes résumées",
        mouvement.len(),
        points.len()
    );
    Ok(())
}
